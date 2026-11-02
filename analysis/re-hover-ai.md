# OPUS RE (loop lane 2, 2026-10-03, card HAIS) - [O] UNAUDITED: no Fable pass
# RE: how a bot drives a hover — the hover AI's own part, the shared steering's hover paths, and Endor's lost bikes

**The question** (card HAIS). `re-walker-ai.md` transcribed the steering the walker and the hover share and left the
hover's own AI `FUN_00220528` unread beyond its call. What does it add, what sticks does a bot's hover get, and how
many speeder bikes do the PS2's own bots lose on Endor (card VDROP's acceptance 1)? All three are answered, and the
law is replayed tick by tick against PS2 RAM in both games (§10).

**Method.** Each function was read in the B2 decomp and settled in the disassembly
(`scripts/tools/eedis2.py`; dumps `lead_tmp/hais/dis_*.txt`, `lead_tmp/wai/dis_b2_*.txt`). Four live PCSX2
runs (software renderer, booted from the Instant Action menus so every hover is sampled from the level's first tick,
the player left on the team screen: every soldier a bot) logged each hover every game tick with its AI's orders beside
the sticks (`lead_tmp/hais/hai_sample.py`, BF1 `hai_sample1.py`); `hai_check.py` / `hai_check1.py` replay the
transcription and `hai_loss.py` counts the losses.

Hops: the controllable's slot +0xcc thunk FUN_00224768 → FUN_00220528 (FUN_002bbf00, FUN_002a95a8, FUN_00224ed0,
FUN_00161c60, FUN_002bb1b0, FUN_002ba940 → FUN_002bd8c0, FUN_002b6258) → FUN_002a54d8 (FUN_002a63e0) → FUN_002a45a0
(FUN_002a42d8, FUN_002a4378) / FUN_002a50c0 (FUN_002a4400); the stuck test FUN_002f1778 → FUN_002f24e8 (FUN_002f2448);
the goal FUN_002f1d80 → FUN_002a5fa8; the mover's bit 4 FUN_002d8f30 → FUN_002db6f0; BF1's FUN_001f2428 →
FUN_001eedc0 → FUN_00233500 (FUN_00234e78, FUN_0023b860, FUN_0023b6e8, FUN_0023b880) → FUN_00232b78.

**Conventions.** As `re-hover.md` and `re-walker-ai.md`: E = the hover, K = its class (E+0x4f4), ctl = E+0x270, sticks
E+0x2dc throttle, E+0x2e0 strafe, E+0x2e4 turn, E+0x2e8 pitch. M = the body matrix E+0x120 (it holds the visual tilt),
rows **X** (+0x00), up (+0x10), fwd (+0x20), pos (+0x30); X = up × fwd is the vehicle's LEFT in the port's terms
(`ai::ride::steer_to`'s `right` = −X). `ai` = the UnitController at E+0x324, `mv` = ai+0x430 its mover, `m` = the
active path follower (`*(*(mv+0x1c)+0x10)`, 0 = none). Tags: [V] read at the cited lines, [D] derived, [H] guess,
[O] = load-bearing and verifier-SUPPORTED (§13).

## 0. The answers [O]

| question | answer |
|---|---|
| what FUN_00220528 adds to the shared steering | Five rules (§2): an **obstacle probe** (a ray 1.5 × speed ahead; a hit moves the goal a quarter of that length sideways and halves the throttle), a **throttle floor** of ±0.5 while turning for a `MovingTurnOnly` class, an **unstick manoeuvre** (reverse at full throttle with half turn until the nose has swung 90°, then forward for 3 s; at most once in 15 s), the turn stick × 0.6 when the throttle is under 0.1, and the 20 % bail-out. |
| the sticks | Throttle and strafe are FUN_002a54d8's pair toward the goal (§3): with a StrafeSpeed, inside atan(StrafeSpeed / ForwardSpeed) of the goal throttle = 1 and strafe = tan(error) × FS / SS, outside it strafe = ±1 and the throttle falls to (SS / FS) / tan(error); without one, throttle = cos(error), strafe 0. The turn is FUN_002a45a0's hover path (§4): u = clamp(s × (4.5 − clamp(TurnRate, 1, 3)), ±1), turn = u·\|u\|, s = the sine of the heading error (±1 when the goal is behind). All four sticks are written raw. |
| what it reads | The body matrix and velocity of the tick before, the mover's order of this tick: mover → AI → drive (§10). |
| how close it replays | BF2 Endor 22617 of 22621 ticks, Kashyyyk 15815 of 15832, Geonosis 6491 of 6492 equal on throttle, strafe and turn; BF1 Endor 24089 of 24222 (§10). |
| speeder bikes lost on Endor, PS2 | **BF2: 4 with a rider in 5.00 min, none by crashing** (19 rides). **BF1: 16 with a rider in 5.00 min, 15 of them crashes** (54 rides; 3.0 a minute). The law is the same in both games; BF1's bike is the difference: 40 m/s, no strafe, and a collision damage of (normal speed)² against 600 health (§11). |

## 1. Objects and fields [V]

- **E+0x584** the unstick state (0 / 1 / 2), **E+0x590** the forward it started with, **E+0x5a0** the stamp of state 2,
  **E+0x5a4** the stamp of the last manoeuvre's end. Written only by FUN_00220528 and the constructor FUN_00219918
  (state 0, E+0x5a4 = −100.0: B2:130957, 130962).
- **E+8** the entity's class; **class+0x330 = `AISizeType`** (hash 0xaff22806, setter FUN_0026a340 B2:187010-187036:
  SOLDIER 1, HOVER 2, SMALL 4, MEDIUM 8, HUGE 0x10, FLYER 0x20; strings at 0x61fbd8-0x61fc00; an unknown string
  stores 1). Default 1, from the base class ctor FUN_00269d98 (B2:186481); the hover class code never writes it
  (verifier), so the probe is off only for a class whose ODF says HUGE — BF1's MTT and hovernaut; no BF2 hover.
- **K+0xe75 = `MovingTurnOnly`** (hash 0x3c45420e; ctor 0 B2:136718, derive copy B2:136935, setter B2:137495). Authored,
  among the side classes (`assets_baked/classes/*.ron`), by all_hover_combatspeeder (both games; BF2's
  all_hover_bos_combatspeeder too) and cis_tread_hailfire (both games) only; RAM: 1 on the combat speeder and the
  hailfire, 0 on the bike and the fighter tank.
- K+0x670 ForwardSpeed, K+0x678 StrafeSpeed, K+0x680 TurnRate, K+0x688 PitchRate (`re-vehicles.md` §3.1).
- **mv+0xa8 bit 4**: set each update by FUN_002db6f0, state 8 of the behaviour dispatcher FUN_002d8f30 (B2:263044,
  264776; 0x2db794-0x2db7b4). It is the only setter: of the 33 `sd` stores to +0xa8 / +0x4d8 off a non-stack base in
  the ELF, one ORs in 0x10 (`bit4_scan.py`; the verifier's own scan agrees). Default 0; cleared by the mover's ctor
  and resets (FUN_002ba3c8, FUN_002ba510, FUN_002ba660: verifier). What state 8 is stays [H] (it re-picks a
  destination when its path ends, flipping a side byte, and aims at its target: a vehicle's attack shuttle).
  **Bit 5** = "stand": set by FUN_002ba810 and by six sites in the behaviour states, read only by the steering.
- **m+0x7c** the path follower's stuck word (§7); FUN_002ba940(mv) = m != 0 and m+0x7c != 0 (0x2ba940-0x2ba968,
  0x2bd8c0-0x2bd8d8).

## 2. The hover AI — FUN_00220528(dt, E) (B2:135537-135870; 0x220528-0x220c50) [O]

Reached only through the thunk FUN_00224768 (B2:138369), slot +0xcc of the hover controllable vtable 0x61ca80 (word
0x61cb4c) and of the commandhover family's (word 0x62b7e4).
*(decompiler excerpt omitted from the public copy)*
- **The probe's ray** [V] the arguments; [D] what it tests: mask 0x43b = the object layers 1, 2, 8, 0x10, 0x20, 0x400
  of `re-collision-layers.md` (soldiers 2, vehicles 8, buildings and props 0x10, riders 0x20) and NOT the terrain's
  0x80; the last argument 1 restricts a hit object to its ordnance bodies. It starts 1 m above the body's origin and
  runs level (dir.y is 0.0001 before the normalize). The second ignored entity comes from E+0x32c (or 0 by E+0x3b0):
  [H] the pilot.
- **The probe moves a COPY of the goal.** mv+0x20 is untouched; next tick starts from the mover's point again. It
  runs above 3.33 m/s (L > 5).
- **An invalid goal holds the sticks** [D]: FUN_002a54d8 then writes neither throttle nor strafe, the locals keep the
  last tick's values and still pass H6-H9. Not seen live (Kashyyyk's three goal-less ticks are input resets:
  verifier).
- **The 15.0 s and the entry rule rest on the code alone**: the BF2 captures hold four manoeuvres (two fighter tanks,
  two hailfires), each a first one; state 2 lasted 3.003-3.008 s on all four. In the tick that sets state 2 the sticks
  are already +1.0 / 0.5.
- **The unstick turn is always the same way** (trn = +0.5 in both states) [V]. The drive's yaw target is −rate · trn
  whichever way the hover travels (`re-hover.md` §6.10), so it backs up turning one way and then drives off still
  turning that way.
- **The order of type 1** [V] the call; [H] its effect: the UnitMobileController maps order type 1 to its state 1
  (FUN_002b8d60, B2:240402-240415), which was not read.

## 3. The steering for a hover — FUN_002a54d8 (0x2a54d8-0x2a5b50; `re-walker-ai.md` §3) [O]

The hover's call is `(dt, ForwardSpeed, StrafeSpeed, TurnRate, PitchRate, ctl, goal, aim, &thr, &str, &trn, &pit)`.
```
hasStrafe = StrafeSpeed > 0.0001                                                   0x2a54e8-0x2a5574
goal.valid == 0: thr and str untouched; noGoal = 1; slowing = 1 → tail
d = goal.pos − M.pos, d.y = 0;  R = ForwardSpeed · 0.5  (FUN_002a63e0, kind 6: 0x2a641c-0x2a646c)
inside = d.x² + d.z² < R²;  slowing = inside and m == 0                            0x2a5608-0x2a5658
thr = 0, str = 0, noGoal = 1 when |d.x| and |d.z| < 0.0001, or when slowing and !hasStrafe   0x2a565c-0x2a5708
dir = normalize(d);  F = M.fwd flattened, unit;  cross = F × dir;  dot = F·dir     0x2a5770-0x2a5850
scale = slowing ? clamp(|d| / R, 0, 1) : 1                                         0x2a5868-0x2a58b4
hasStrafe:   a = dot / ForwardSpeed;  b = −cross.y / StrafeSpeed                   0x2a58b8-0x2a5964
    |b| < |a|:  str = (b / |a|) · scale;  thr = a <= 0 ? −scale : scale
    else:       thr = (a / |b|) · scale;  str = b <= 0 ? −scale : scale
!hasStrafe:  thr = clamp(dot, 0, 1) · scale;  str = 0                              0x2a5968-0x2a59e8
tail (an EntityHover): useAim = slowing and hasStrafe, else bit 5 of mv+0xa8       0x2a59ec-0x2a5a50
    useAim or noGoal:  FUN_002a50c0(dt, TurnRate, PitchRate, ctl, goal, aim, &trn, &pit, 0)    (toward the aim point)
    else:              FUN_002a45a0(dt, TurnRate, PitchRate, ctl, &goal.pos, &trn, &pit, 0)    (toward the goal)
```
So a hover with a strafe speed slides and turns together; a strafe-less hover that is inside R with no path left
stops; and inside R a strafing hover fades its sticks by |d| / R and turns its nose to its aim.

## 4. A stick toward a point, the hover path — FUN_002a45a0 (0x2a45a0-0x2a4cc8) [O]

An EntityFlyer, an EntityHover, or a MountedTurret whose rate argument is over 0.5 takes this path (0x2a4624-0x2a46d0).
- **Yaw** (0x2a4680, 0x2a46d4-0x2a4830):
  ```
  d = target − M.pos;  d.y = 0
  l = (d·X, d·up, d·fwd)                       FUN_002a42d8: the body frame, tilted rows
  l = normalize(l.x, 0, l.z)                   FUN_002a4378: (1, 0, 0) when both are 0
  t = −l.x;   if l.z < 0:  t = (−l.x < 0) ? −1 : +1
  u = t < 0 ? max(−1, t · k) : t > 0 ? min(1, t · k) : 0,     k = 4.5 − clamp(yawRate, 1, 3)
  turn = u · |u|
  ```
  No dt and no division by the rate: unlike the walker's deadbeat stick this is a shaped proportional stick, full at
  |l.x| ≥ 1 / k (k = 1.5: 41.8°; k = 2.5: 23.6°; k = 2.8: 20.9°) and quadratic below it.
- **Pitch** (0x2a4958-0x2a4b98): the aim frame is built from the aim direction ctl+0xe0 and the world up
  (DAT_0073d8a0): right = normalize(up × aim), up' = aim × right, origin ctl+0xd0 (FUN_001bc7c0). With la = the unit
  direction to the target in that frame: w = clamp(la.y · (5.0 − clamp(pitchRate, 1, 3)), ±1) by sign as above,
  pitch = w · |w|.

The aim point FUN_002a50c0 is `re-walker-ai.md` §5 unchanged; for a hover its R is ForwardSpeed · 0.5. With no aim order
(type 2) it is M.pos + normalize(goal.pos − M.pos) · 30 when the goal is valid and outside R, else M.pos + M.fwd · 30
(0x2a51f4-0x2a52f0; FUN_002a4400 is a 3-component normalize).

## 5. The goal and arrival [O]

- **Look-ahead**: for an EntityHover the mover's point is FUN_002a5fa8(|velocity|, m+0x10, m+0x20, M.pos)
  (B2:281227-281243): with A, B the segment's ends at the unit's height and u its direction, the point is
  A + u · (u·(pos − A) + |v| + 0.2), or B once that passes the segment's end. Live: equal within 2 cm on 16032 of
  16209 Endor ticks and 20065 of 20078 Kashyyyk ticks that had a path segment (pos and v of the tick before).
- **Arrival**: FUN_002a63e0 at the path's last node and FUN_002a6318 at the others both give ForwardSpeed · 0.5 for a
  hover (0x2a641c-0x2a646c, 0x2a6358-0x2a638c) — 14 m for BF2's bike, 20 m for BF1's, 4.5 m for a fighter tank.

## 6. Boost [V]

`re-hover.md` §5.1 I5: an AI's hover boosts while mv+0xa0 == 3. Live on Kashyyyk the mode read 3 on 263 of 36574 AI
ticks (two fighter tanks, a combat speeder and a bike) and 1 on 4606; on Endor and Geonosis always 0. Its senders
stay as `re-walker-ai.md` §8 left them [H].

## 7. The stuck test — FUN_002f24e8(dt, m) (B2:281420-281538; 0x2f24e8-0x2f2708) [O]

Called from the path follower's update FUN_002f1778 only (B2:280931). (a, b, c) = (2.0, 3.0, 5.0), a droideka
(2.0, 6.0, 6.0) (FUN_002f2448).
```
m+0x7c = 0
if |M.pos − m+0x20|²(x, z) < 0.04: return                         (at the node)
if |m+0x80 − M.pos|² > a²: m+0x80 = M.pos; m+0x90 = 0             (moved 2 m from the anchor)
else:                      m+0x90 += dt
if m+0x90 > b: m+0x7c = 2                                         (not 2 m in 3 s)
if |m+0x20 − M.pos|² < m+0x94: m+0x94 = that; m+0x98 = 0          (a new closest distance to the node)
else: m+0x98 += dt;  if m+0x98 > c and m+0x7c == 0: m+0x7c = 1    (none for 5 s)
if m+0x7c != 0: send the controller message 7 with the code
```
So H8's entry: the hover has a path, and either it has not moved 2 m in the last 3 s or it has not come closer to
its node for 5 s — and 15 s have passed since its last manoeuvre ended.

## 8. What the law does on the shipped classes [D]

Port terms: n = the flat unit direction to the goal, s = n·right (> 0 = goal to the right), c = n·fwd; then
b = s / SS, a = c / FS, l.x = −s (in the tilted body frame), and a positive strafe or turn stick moves / turns right.

| class | FS | SS | TurnRate | k | strafe band atan(SS / FS) | R | MovingTurnOnly |
|---|---|---|---|---|---|---|---|
| BF2 imp_hover_speederbike, rep_hover_barcspeeder | 28 | 3 | 5.0 | 1.5 | 6.1° | 14 | – |
| BF2 fighter tanks | 9 | 6 | 1.7 | 2.8 | 33.7° | 4.5 | – |
| BF2 all_hover_combatspeeder | 11 | 7 | 1.7 | 2.8 | 32.5° | 5.5 | yes |
| BF2 cis_tread_hailfire | 15 | 0 | 1.5 | 3.0 | – | 7.5 | yes |
| BF1 imp / rep_hover_speederbike | 40 | 0 | 2.0 | 2.5 | – | 20 | – |

(FS / SS / TurnRate / flags read from the captures' RAM for the five classes sampled; the BARC from the disc.)
- **A BF2 bike** more than 6.1° off its goal holds strafe ±1 and a throttle of 0.107 / tan(error) — 0.29 at 20°,
  0.11 at 45° — so it slows hard to turn; live median speed 12.9 m/s against ForwardSpeed 28.
- **A BF1 bike** has no strafe: throttle = cos(error), 0.5 still at 60°; live median speed 29.0 m/s, p90 39.9.
- The probe looks 42 m ahead at 28 m/s and 60 m at 40 m/s; a hit moves the goal 10.5 m / 15 m sideways for that tick.

## 9. BF1 [V]

FUN_001eedc0 (B1:100342-100714; 0x1eedc0-0x1ef4c0; thunk FUN_001f2428 B1:102834; a second thunk FUN_0029b438
B1:225169 has no reference of any kind) is the same function with these differences:
- no health test (no bail-out);
- the unstick cooldown is **4.0 s**, not 15.0 (B1:100665; 0x1ef364 `lui 0x4080`); state 2 is 3.0 s as in BF2
  (B1:100702) and posts no order when it ends;
- no × 0.6 on the turn at a low throttle (the function ends at the stores, B1:100710-100712);
- the MovingTurnOnly gate is the word E+0x3d8 (B1:100639), not a mover bit — default 0, set to 1 by FUN_0024ef38,
  which sits in three goal vtables (verifier); the class flag is K+0x27e8 (B1:100634), the same property hash;
- the probe is the same (AISizeType at class+0x78 != 0x10, L = 1.5 · speed > 5.0, shift 0.25 · L, k = 0.5; B1:100443-
  100613) with ray mask 0x21f (FUN_00161d68, B1:100542).

BF1's steering FUN_00233500 (B1:148115): R = ForwardSpeed · 0.5 for a hover (FUN_00234e78, B1:149329); **no stop for a
slowing strafe-less hover** (only the walker and droideka stops exist); the |d| / R scale also applies inside R WITH a
path when the path follower's word m+0x8c is set (FUN_0023b6e8); "stand" is the word ctl+0x13c (E+0x3dc). BF1's
stick function FUN_00232b78 has the hover yaw of §4 instruction for instruction (0x232c50-0x232db0: 3.0, 4.5, the
square), for a flyer or a hover.

Live (BF1 Endor): the one bike that ran the manoeuvre re-entered it twice 4.004 s after the previous one ended, and
state 2 lasted 3.003 s both times it ended alive — the 4.0 and the 3.0 as read.

## 10. The PS2 itself [O]

`lead_tmp/hais/hai_run.sh <row> <tag> <seconds> [era]` / `hai_run1.sh`; per tick the last read before the game clock
moves is kept. Each tick's sticks are predicted from the matrix, velocity and aim frame of the tick before and the
mover's orders of the tick. The other orders fail on BF2 Endor: the matrix of the same tick gives 35.10 % (all three
sticks), the goal of the tick before 58.18 %, the aim frame of the same tick 79.93 % on the pitch — so **the mover
sets the goal, the AI writes the sticks from the body of the tick before, then the drive moves the body.**

| capture | classes | ticks replayed | throttle, strafe, turn all equal (2e-3) | of those, fitting only with a probe hit | pitch equal |
|---|---|---|---|---|---|
| BF2 Endor, 300 s | speeder bike ×9 | 22621 | 22617 (99.98 %) | 972 | 22618 |
| BF2 Kashyyyk GCW, 240 s | bike, fighter tank, combat speeder | 15832 | 15815 (99.89 %) | 634 | 15827 |
| BF2 Geonosis, 240 s | hailfire ×2 | 6492 | 6491 (99.98 %) | 153 | 6483 |
| BF1 Endor, 300 s | speeder bike ×9 | 24222 | 24089 (99.45 %) | 3598 | not replayed |

- **Not in the table**: ticks the sampler read fewer than three times (it caught them before their updates ran: 64 /
  186 / 8 / 37); ticks turning at an aim order of type 0 / 1 (the lead and error vector of `re-walker-ai.md` §5 are
  not replayed) — there throttle and strafe alone are compared: equal on 2524 of 2524 (Endor), 20090 of 20222
  (Kashyyyk), 1209 of 1330 (Geonosis); those misses are not broken down (H6 and H8 need the turn and are not applied
  there).
- **The probe cannot be replayed** (a ray into the PS2's collision world). A tick is tried without a hit, then with a
  hit on either side; a tick that fits only with a hit must agree on all three sticks to 2e-3 with a goal moved
  L / 4 sideways and the throttle halved. That is the weaker evidence: 4.3 % of BF2 Endor's replayed ticks, 14.9 %
  of BF1 Endor's.
- **Branches seen, all three sticks equal / ticks** (BF2, three captures, 44923 / 44945 in all; the groups overlap):
  strafe pair, throttle side 20844 / 20855; strafe pair, strafe side 14572 / 14581; with the |d| / R scale
  6440 / 6440; turning to the aim point 9949 / 9949; stopped 3064 / 3064; strafe-less 5969 / 5970 and with the
  MovingTurnOnly floor 1861 / 1861 (Geonosis); unstick state 1 200 / 200, state 2 270 / 270, the 1 → 2 tick 3 / 3
  (Kashyyyk, Geonosis); the unstick state itself predicted on every replayed tick. H9's × 0.6 shows as turn = ±0.6
  at a throttle under 0.1. H2: on 313 of 315 ticks under 20 % health with (ai+0x5a0 & 3) == 0 the four sticks did
  not change.
- **Not seen**: an AISizeType HUGE hover (none ships in BF2).
- **The misses** (4 + 17 + 1) are single ticks where the body was moved between the two samples (a parked bike nudged
  4.5 cm) or one stick is off by 0.002-0.006; BF1's 133 are all inside R with no path, where the goal is the bike's own
  position to a millimetre and the direction to it is noise.
- **BF1**: the path follower's two words (m+0x8c, m+0x6c) were not sampled; the scale is tried both ways (it mattered
  on 1 tick) and the unstick's entry is taken as read. Unstick ticks 504 / 504.
- **What the replay tests** (the verifier's `verify/mutation_control*.py`, re-run by the lead: one input falsified at
  a time, all three sticks equal). BF2 Endor / Kashyyyk / Geonosis: the path flag inverted 57.7 / 74.5 / 85.6 %;
  bit 5 forced on 77.0 / 47.0 / 73.3 %; ForwardSpeed × 1.25 18.1 / 16.3 / 99.0 %; StrafeSpeed 0 13.4 / 13.0 / (it is
  0) %; TurnRate + 0.5 (no change: clamped) / 65.4 / 53.2 %; MovingTurnOnly flipped (no change) / 99.6 / 85.5 %; the
  stuck word forced 0: 99.98 / 99.88 / 99.95 % — the unstick's entry is the thin part. BF1 Endor with BF2's stop rule
  84.5 %, with BF2's × 0.6 rule 82.1 %. Of BF2 Endor's 14311 ticks where the probe ran and the no-hit reading fits,
  a hit reading fits too on 91 (0.6 %; `verify/probe_control.py`), so the three-way search is not what makes the
  replay fit; 972 of the 15283 probe ticks fit only with a hit (6.4 %).

## 11. Speeder bikes lost on Endor — the PS2's own bots [O]

`hai_loss.py`: a loss = health reaching 0; a rider = an AI controller (BF1: the pilot pointer) on the last live tick.
The killing blow gets no contact response (`re-hover.md` §7: K7 runs only while the alive bit is set, and the damage
call comes first), so it is a crash when the bike was moving at 10 m/s or more and the amount is one the class's
collision damage can deal (at most CollisionScale × (2 × speed)²).

| | BF1 END1 | BF2 END1 |
|---|---|---|
| battle sampled | 5.00 min from the first tick, 16 v 16 bots | 5.00 min, 16 v 16 bots |
| bike-minutes with an AI aboard | 13.5 | 14.0 |
| rides (an AI attaches … detaches) | 54 | 19 |
| **bikes lost with a rider** | **16 (3.2 a minute)** | **4 (0.8 a minute)** |
| … by crashing | **15 (3.0 a minute; 0.28 a ride)** | **0** |
| … by a weapon | 1 (80.0 hp at rest) | 4 (1000.0, 450.0, 450.0, 100.0 hp) |
| empty bikes lost | 2 (60 hp a second, at rest) | 3 (8 hp a second, at rest) |
| median speed with an AI aboard | 29.0 m/s | 12.9 m/s |

- **BF1's fatal blows are the bike's own collision damage.** 15 killing drops at 27-41 m/s: 1689.8 hp at 41.1 m/s
  (= 41.1²), 1373.0 at 39.7, 1281.0 at 37.3, 1022.5 at 37.9, 975.3, 920.2 at 30.3 (= 30.3²), 879.3, 733.7, 463.8,
  437.9, 418.3, 331.6, 230.6, 223.1, 152.5. The BF1 bike authors `CollisionScale 1.0 −0.5 0` and no threshold: damage =
  s² · (1 − 0.5 · n·up) against MaxHealth 600 — any hit over 24.5 m/s normal speed kills a fresh bike.
- **BF2's bike** authors `CollisionThreshold 4 20 0`, `CollisionScale 0 −0.0001 −0.1`, MaxHealth 80: a head-on hit at
  28 m/s costs 57.6. Its three live impacts (velocity jumps of 12-14 m/s) cost 3.3, 0.9 and 0.7 hp.
- **The crash rule's limits** (verifier). On BF1 the amount bound is over 4500 hp, so the rule is in effect "killed
  while moving at 10 m/s or more"; the velocity change on every killing tick is 0.8-1.6 m/s. What separates the two
  kinds in the data: 14 of the 15 crash amounts are fractional and √amount is at or under the bike's own speed on 14
  of the 15, while the drops at rest are whole numbers (60 hp 53 times, 80 three times, 30 three times; the three
  kills at rest are 60, 60 and 80). In BF2 the damage function returns at once when the other body is a hover or a
  walker (verifier), so the bound's "two hovers head-on" is generous there.
- **Two of BF1's 15 crash kills happened inside the unstick manoeuvre**: bike 0x1e8b0b0 entered state 2 at 111.16 s
  and died at 112.69 s (31.6 m/s), and again 258.77 → 261.11 s (40.2 m/s) — three seconds of full throttle with half
  turn, the probe's halving overwritten. Three more died within 7 m of (−161, −4, 294).
- An empty bike at rest loses a tenth of its health each second until it is gone (both games; not traced here).
- One run each; the player was never in the battle (`ps2-samples-without-player-slow-think`: the soldiers then think at
  their slowest interval — the driving law runs every tick regardless).
- **The port, one seed** (`round_to_outcome 60000 42`, main's binary of 00:33, `lead_tmp/hais/port/`): bf1_end1 a 419 s
  round, speeder bikes destroyed 0 killed / 29 unowned (4.2 a minute); bf2_end1 376 s, 2 killed / 40 unowned (6.4 a
  minute). "Unowned" counts empty bikes too; the PS2's comparable totals are 3.6 and 1.4 a minute.

## 12. For the build (card VDROP)

1. **The driver's sticks** (§2-§4 in the port's terms, §8): throttle / strafe by the pair rule or cos(error); turn =
   u·|u|; raw and unquantized as for the walker (`HoverInput { human: false }`). Body frame = the tilted rows.
2. **R = ForwardSpeed / 2** for the stop / scale test and for arrival at every node; the goal |v| + 0.2 m ahead along
   the leg (the port's straight-at-the-node goal is OURS).
3. **The probe** needs a ray against soldiers, vehicles and buildings / props (not terrain) from 1 m up, L = 1.5 |v|
   when over 5 m. PROVISIONAL in the port until its ray set matches the PS2's mask (the ordnance bodies).
4. **The unstick manoeuvre** with the port's own stuck signal standing in for §7 until the path follower has one
   (2 m in 3 s; no new closest distance in 5 s). BF1 rules: cooldown 4 s; BF2: 15 s.
5. **MovingTurnOnly floor**: bit 4's sender is [H]; the port has no such behaviour state — leave it out, PROVISIONAL.
6. **Acceptance numbers**: §10's rows (`hai_check.py --rows` writes PS2 rows for a unit test) and §11's table. VDROP's
   "BF1 END1 under 5 a round" is not the PS2: its own bots crash 15 bikes in 5 minutes there.

### 12b. What the port does after VDROP (2026-10-03)

`core/src/ai/hover_drive.rs`: §2 H3-H10, §3, §4 and §5's look-ahead as transcribed; §7's stuck test on the vehicle
(`HoverAi`); BF1's rules (§9) for a BF1 map's classes. Its test replays `lead_tmp/hais/ps2_rows.csv`: 347 of 347 rows
within 2e-3 on four sticks and the unstick state (worst 2.4e-5), 15 branches, 162 rows with a probe hit. The port's
own: the probe's ray set (the ORDNANCE bodies and other hulls, no soldiers); the segment's start when a route begins
or changes; no MovingTurnOnly floor, no boost, no bail-out; the pitch stick; the dismount.

**Endor's log trap was the BF2 loss.** `end_weap_treesmash` (base `trap`, both Endors, two instances) has two log
cylinders parented to bones out in the trees; the map bake keeps them bone-relative and the port laid them crossed on
the ground at the instance, on the graph's line. 160 of the 171 riders lost with no killer in 8 BF2 rounds died
there, grinding at under 4 m/s (the class's threshold 4 + 20·n·up falls to 0 against a normal 11.5° under the
hull's plane, so every tick dealt 0.8 hp of 80). The PS2's bikes pass 0.3-0.8 m from both origins at 28 m/s (37
passes in 5 min, `live_end1.csv`) and BF1's at 0.44 m, 39.5 m/s; of the PS2's 9933 AI ticks under 4 m/s none lost
health to a contact. The port now gives a `trap` no body (`game/src/assets.rs`, PROVISIONAL until the trap is built).

8 seeds, `round_to_outcome`'s `hovers lost` line (`lead_tmp/vdrop/hov.py <sweep dir>`), speeder bikes:

| | PS2 (5 min, §11) | main before | the law, the logs still there | the law, no logs (landed) |
|---|---|---|---|---|
| BF1 END1 crashes with a rider per ride | 0.28 | 0.53 (187 / 351) | 0.23 (76 / 326) | 0.19 (74 / 384) |
| … a minute | 3.0 | 3.05 | 1.33 | 1.27 |
| BF2 END1 crashes with a rider a round | 0 | 16.9 | 1.4 | 0.2 (2 in 8 rounds) |
| BF2 END1 riders lost with no killer, 8 rounds | 0 | 231 | 171 | 3 |

A crash = lost with a bot aboard, no killer, at 10 m/s or more the tick before. All 32 maps: 549 crashes in 3561
rides before, 225 in 3753 after. The port already has the empty hover's decay (`Sim::decay_pad_vehicle`; the "empty at
rest" column), so "unowned" was never crashes alone.

Not turned on: the hub nearest the post for a hover whose graph ends short (`ai::HOVER_NEAREST`). Measured on: 43 %
more rides (5366), the same crash rate a ride, but tat1's vehicle deaths rise 5.6 a round and on BF2 POL1 22 riders in
8 rounds get out over no floor. Neither Hoth has a hover class (tauntauns, AT-STs, AT-ATs, snowspeeders).

## 13. Checks run, the verifier, what stayed [H]

Outputs in `engine/artifacts/lead_tmp/hais/`: `check1_lines.txt`, `checks.txt`, `check_*.txt`, `loss_*.txt`, `verify/`.
- **Check 1** (`re_check.py fn`): every B2 / B1 line cited here lies in the function its section names.
- **Check 2 / 5** (absence, writers): FUN_00220528 has one caller (the thunk) and the thunk two data words; E+0x584 /
  0x590 / 0x5a0 / 0x5a4 are stored only by FUN_00220528 and the ctor; K+0xe75 has three writers (ctor 0, derive,
  setter) and one reader; mv+0xa8 bit 4: one setter in an instruction scan (`bit4_scan.py`); FUN_002f24e8 one caller.
- **Check 3** (twins): B1 FUN_001eedc0, FUN_00233500 and FUN_00232b78 opened (§9) and replayed (§10).
- **Check 4** (bytes): the constants are immediates, read in the disassembly: 0.2 (0x3e4ccccd), 1.5 (0x3fc00000), 5.0
  (0x40a00000), 0.0001 (0x38d1b717), 0.25 (0x3e800000), 0.5, 15.0 (0x41700000), 3.0 (0x40400000), 0.1 (0x3dcccccd),
  0.6 (0x3f19999a), 4.5 (0x40900000), 0.04 (0x3d23d70b), 2 / 3 / 5 / 6 (FUN_002f2448); BF1 4.0 (0x40800000).
- **To re-run:**
  ```
  cd engine/artifacts/lead_tmp/hais
  python3 hai_check.py live_end1.csv end1_end.p2s | grep ^TOTAL      # 22621 replayed, all three 22617
  python3 hai_check.py live_kas2.csv kas2_end.p2s | grep ^TOTAL      # 15832 / 15815
  python3 hai_check.py live_geo1.csv geo1_end.p2s | grep ^TOTAL      # 6492 / 6491
  python3 hai_check1.py live_b1end1.csv b1end1_end.p2s | grep ^TOTAL # 24222 / 24089
  python3 hai_loss.py live_b1end1.csv | head -4                      # 18 losses, 16 with a rider, CRASH 15
  python3 hai_loss.py live_end1.csv | head -4                        # 7 losses, 4 with a rider, CRASH 0
  python3 bit4_scan.py                                               # 33 stores, 1 hit: 0x2db7b4
  ```
- **Verifier**: one Opus agent, foreground, `verify/claims.md` only (49 claims) → `verify/VERDICTS.md`: **49
  SUPPORTED, 0 PARTLY, 0 NOT SUPPORTED.** Every cited range re-disassembled, every immediate matched, the absences
  attacked with its own scans (`verify/scan_*.py`), the data re-run and the replay attacked with mutations. Its
  refinements are folded in: the probe's vector is E+0x4c0; AISizeType's default; bit 4's and bit 5's clearers and
  setters; the invalid-goal case; the dead BF1 thunk; BF1's gate word's setter; the unstick's thin live coverage; the
  crash rule's limits; the two crash kills inside the manoeuvre. Not covered by its scans: a store through a pointer
  computed more than 24 instructions earlier, or a block copy.
- **Stayed [H]:** what behaviour state 8 is; the second entity the probe ignores; what order type 1 makes the
  controller do; mode 3's senders; the empty bike's decay; BF1's m+0x8c; BF2's 15 s cooldown live (no second
  manoeuvre was captured); the probe's ray against the port's collision world (never compared); how much of BF1's
  crash rate the missing player changes (one run each, bots only).
