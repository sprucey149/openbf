> **CORRECTED 2026-10-03 (card DEKA, `re-droideka-unit.md` section 0):** (1) the ball's ground speed is TWICE the
> values below - the entity integrates twice a tick; measured top 12.96 m/s (BF1) and 11.99 m/s (BF2), and it falls at
> 39.2 m/s^2 (the jump's apex stays 1.27 m, in 0.51 s). (2) There is no ground probe: keys 0xf01d5583 / 0xac229931 are
> water-damage depths and the "fall-anim timer" is the water timer. (3) Main vtable +0x10c (BF1) / +0x12c (BF2) is the
> human's camera / zoom, not weapon fire. (4) BF1's transform times are measured: fold 0.50 s, unfold 1.67 s.

# LEAD VERIFICATION (Fable lead, 2026-09-07, session 13)
Status: **VERIFIED** — six load-bearing claims re-read by a Fable verification agent
(`engine/artifacts/lead_tmp/s13/reports/droideka_verify.md`, all CONFIRMED) and the lead re-read
BF2 `FUN_001f7110` 108096–108098 (the `(+0x2b4 & 3) == 3` jump edge → vtable +0xb8/+0xac), the loader
112712–112716 (`BallJumpHeight` → `sqrt(2·h·9.8)` into class +0xebc), `FUN_001f3360` 105142
(`vel.y += class+0xebc`) and BF1 `FUN_001ce828` 77802–77808 (`v ← v − 2(v·n)n` when `v·n < 0`) in
`analysis/SWBFRNT2.ELF-decomp.c` / `SWBFRONT.ELF-decomp.c`. Corrections to the text below, from the
verification: (1) §"only writers of velocity.y" is incomplete — 76511 (ctor), 77808 (the mirror bounce)
and 77838 (contact-frame rolling velocity on a slope) also store y; none is a jump, the conclusion stands.
(2) §2 "when the contact slope exceeds MaxBallAngle" is inverted: B1:77816 tests `class+0x28a0 < n.y`
(surface flat ENOUGH). (3) BF1 soldier slot +0x80 holds the thunk `FUN_00210ec8` → `FUN_00208b78`
(cosmetic). (4) The transition-table compare is 448 bytes (14×8×u32); the report's `elfrd.py … 112` reads
112 words, so its compare was right. (5) Open: whether the PS2 pad delivers the +0x2b4 edge to a
droideka-driven Controllable (same field/offset as the soldier's — very likely; PCSX2 is the only closure).
**Consequence for PLAN §Movement & feel:** the maintainer's line names the wrong game — BF2's ball HAS the button
jump, BF1's has NONE (its only upward impulse is the PostCollision mirror bounce). Decision brought to
The maintainer in the PLAN Log 2026-09-07. Port spec, if the universal ball jump stands: BF2's numbers —
BallJumpHeight 1.27 m (v0 = 4.99 m/s), EnergyCostJump 30 of EnergyBar 100 (restore 10/s, 15 idle),
grounded < 0.1 s, +25 % along facing, Rolling state only.
Tags: [V] where the verification quotes the line; speeds 6.0 / 6.5 m/s are [D] (BallMoveSpeed·BallRadius).

# RE: droideka (`walkerdroid`) ball form and jump — BF1 vs BF2

Corpora: `analysis/SWBFRONT.ELF-decomp.c` (BF1, "B1:" line numbers) and
`analysis/SWBFRNT2.ELF-decomp.c` (BF2, "B2:"); vtables/tables dumped from the ELFs
(`extracted/bf1/SWBFRONT.ELF`, `extracted/bf2/SWBFRNT2.ELF`). Hashes = fnv1a over
`byte|0x20` (`classdb.rs::fnv1a_lowercase`). Tags: [V] read in the decomp / dumped
bytes, [D] derived (arithmetic or naming inferred from use), [H] hypothesis.

## §0 Headline

1. **The premise is inverted by the code.** BF2's droideka ball HAS a button jump
   (`BallJumpHeight` = 1.27 m in the PS2 BF2 ODF → v0 = √(2·9.8·1.27) = 4.99 m/s,
   costs `EnergyCostJump` 30 of `EnergyBar` 100, only when grounded < 0.1 s,
   +25 % forward boost). BF1's droideka has NO jump in any form: no jump property in
   its loader or its CIS.LVL ODF, the jump slot of its Controllable vtable is the base
   `jr ra` stub, and no code in the EntityDroideka region ever adds to velocity.y
   except gravity. What BF1's ball does do is *reflect* its velocity off surfaces it
   drives into (PostCollision), which may read as a "hop" [H].
2. Class, both games: classlabel `walkerdroid` → `EntityDroidekaClass`; the entity is
   `EntityDroideka`, its own class deriving from EntityGeometry + Controllable + Thread
   + Trackable + PlayableEntitySounds — NOT EntitySoldier, NOT a vehicle/walker [V].
   Port placement: a dedicated unit class that shares the soldier's *input/Controllable*
   layer and collision step but has its own 14-state anim machine (walk states 0–10,
   0xb IdleToRoll, 0xc Rolling, 0xd RollToIdle, 4 dead) — identical transition table
   in both ELFs.
3. State rules: transform = one button (BF1 logical button 7 → ctrl byte +0x52; BF2 the
   crouch/roll button +0x2b8); pressed while walking → 0xb→0xc, pressed while rolling
   (and headroom clear) → 0xd→0. Rolling: no firing (both), BF2 additionally has a
   sprint-boost (×2 accel/top speed, drains `EnergyDrainSprint` 30/s). BF2 transform
   times are ODF (`RollTime` 0.50 s, `UnrollTime` 1.66 s, anims rescaled); BF1 uses the
   raw anim lengths (not in the ELF).
4. Ball physics is its own integrator in both (angular speed ω with accel/friction/clamp,
   world v = ω·BallRadius), not the soldier's. Top speed BF2 = `BallMoveSpeed` 6.0 m/s;
   BF1 = BallMoveSpeed·BallRadius = 10·0.65 = 6.5 m/s [D].
5. **Port spec ("ball can jump everywhere")**: implement BF2's jump (B2 `FUN_001f3360`)
   on the universal droideka: state Rolling && grounded(airborne < 0.1 s) && energy
   spend 30 → vy += 4.99 m/s, horizontal along facing ×1.25, airborne timer = 0.1
   (no double jump), `JumpSound`. Numbers from the BF2 ELF/ODF; for BF1-sourced
   droidekas the class lacks BallJumpHeight → PROVISIONAL default 1.27 (BF2 ODF), not
   the BF2 engine default 5.0 m/s (class ctor, = 1.28 m).

## §1 The class

**Classlabel → constructor.** BF1 `FUN_00172ab0` registers the label table: B1:15588-15590
*(decompiler detail omitted)* → the class
object is 0x28f0 bytes, ctor `FUN_001d31d8` (B1:80941), class vtable `DAT_0052ff00`
(B1:80961). Siblings in the same table: soldier→`FUN_00213dd8`, droid→`FUN_001cb208`,
walker→`FUN_0021cca8`, flyer→`FUN_001dead8` [V]. BF2 `FUN_00173d10`: B2:16400-16401
`"walkerdroid"@0x60fb90 → FUN_001fc3a8(alloc(0x10a0), h)`, class ctor B2:111854, class
vtable `DAT_0061b9a0` (B2:111869) [V]. Both class ctors call the shared controllable-class
base ctor first (BF1 `FUN_001e30c8`, also called by the soldier class ctor B1:126042+;
BF2 `FUN_00269d98`, soldier B2:169328) [V].

**Entity.** BF1 entity ctor `FUN_001cd060` (B1:76381): sub-objects Controllable at
+0x2a0 (`FUN_0023a648(this+0x2a0,…)` B1:76421 — same offset as the soldier), Thread at
+0x540, PlayableEntitySounds at +0x5b0; vtables: main `DAT_00530138`, Controllable
`DAT_0052ffc8` (stored at +0x534, B1:76436), collision `DAT_005300a0` (+0x100), Thread
`DAT_0052ffa8`. RTTI `FUN_001d5f78` (B1:82693) = "14EntityDroideka" with 6 bases:
20PlayableEntitySounds, PostCollision, 9Trackable, 6Thread, 12Controllable,
14EntityGeometry [V] — no EntitySoldier. BF2 entity ctor `FUN_001f2618` (B2:104546),
Controllable at +0x270 (vptr at +0x284, vtable `DAT_0061ba60`, B2:104604), main
vtable `DAT_0061bc50` (B2:104600), class ptr at +0x4a4, state at +0x1ad8 [V].

**Controllable action slots (dumped).** BF1 soldier vtable 0x532be0: +0x7c crouch
`FUN_00208b00`, +0x80 jump `FUN_00208b78`, +0x84 prone, +0x8c/+0x90 roll L/R.
BF1 droideka vtable 0x52ffc8: +0x7c..+0x90 = 0x23e8e8..0x23e910 = base Controllable
stubs, each `03e00008 00000000` (`jr ra; nop`) → **the jump action is a no-op** [V];
droideka overrides only +0x2c `FUN_001d2ee0`, +0x30/+0x34/+0x40 getters, +0x60..+0x70
(`FUN_001ce7e8/7f8`, `FUN_001cdbf0`, `FUN_001ce808`), +0x98 `FUN_001d2a08` [V].
BF2 soldier vtable 0x61e998: +0xa4 crouch, +0xac jump `FUN_00255b88`, +0xb0 roll,
+0xb4 sprint, +0xb8 stop-sprint. BF2 droideka vtable 0x61ba60: **+0xac =
`FUN_001ff238` → `FUN_001f3360(this−0x270)` (ball JUMP)**, +0xb4 `FUN_001ff258` →
`FUN_001f3508` (ball boost on), +0xb8 `FUN_001ff278` → `FUN_001f3570` (boost off);
+0xa4 crouch / +0xb0 roll stay base [V] (B2:113637-113661).

**Class property loaders** (class vtable +0x20: BF1 `FUN_001d3f68` B1:81430-82078; BF2
`FUN_001fd2f8` B2:112458-113211). Hash → class field (unhandled hashes fall through to
the base loader via class+0x160/+0x13dc). Preimages by brute force
(`droideka_scratch/brute2.py`, cross-product of ≤3 tokens); unresolved listed as hashes.

| key (hash) | BF1 field (default) | BF2 field (default) | used for |
|---|---|---|---|
| BallMoveSpeed 0xacacdb0c | +0x287c | +0xe8c | ω clamp / top speed |
| BallRadius 0x9895bb7a | +0x2878 | +0xe88 (1.0) | v = ω·r |
| 0xdc524dd9 (unresolved) | +0x2888 | +0xe9c (1.0) | ω units factor, cancels in v [D] |
| BallAcceleration 0xde6b8538 | +0x2894 | +0xea4 (100) | dω/dt = stick²·A |
| BallRollingFriction 0x6560de67 | +0x2898 (2.0) | +0xea8 (2.0) | ω decay on contact |
| BallTurnSpeed 0x578f6ae6 | +0x2884 | +0xe94 (2.0) | yaw rate at full stick |
| 0x7c8f02c0 (BF1) / 0x12a7e73d (BF2) | +0x26c4 (0.1) | +0xe90 (2.5) | yaw rate at zero stick |
| BallMaxLean 0xd2da8f5e | +0x2890 (0.2) | +0xea0 (0.2 rad; deg→rad B2:113117) | lean vector |
| MaxBallAngle 0xe6f7befb | +0x28a0 | +0xeac (0.7 rad, deg→rad) | slope limit in PostCollision |
| BallMoveThreshold 0x9a7a164 | +0x288c | — (BF1 only, B1:81539) | |
| BallCollision 0x8f720e3c | +0x13e4 (prim name) | — | ball collision prim ("p_rollsphere") |
| 0xafe693ce TerrainCollision | +0x13e4 | +0x168 | BF2 swaps to class+0xefc ("coll_ball") on entering 0xc (B2:108250) |
| **BallJumpHeight 0x2daa5a0b** | **absent** | +0xebc = √(2·9.8·h), default 5.0 m/s (B2:112712-112716) | jump v0 |
| 0x388d3934 (unresolved, "1.25") | absent | +0xec0 (1.25) | jump forward factor (B2:112746) |
| EnergyCostJump 0xb4a7fd4 | absent | +0xedc (30) (B2:112577) | |
| EnergyDrainSprint 0x134361b9 | absent | +0xed8 (30) | boost drain |
| EnergyRestore / RestoreIdle | absent | +0xed0 (10) / +0xed4 (15) | |
| JumpSound 0x580dc4ec | absent | +0x1064 (B2:112795) | |
| RollTime 0xc817a0f7 / UnrollTime 0x3209add4 | absent | +0x77c (0.5) / +0x778 (1.67) | anim rate B2:113299-113301 |
| MaxSpeed 0x86df0364 / MaxTurnSpeed 0x5433f589 | +0x1870 / +0x1874 | +0x768 / +0x76c | walk |
| 0xf01d5583 / 0xac229931 ("1.5"/"0.5") | +0x28e0 (1.5) / +0x28e4 (0.5) | +0x1078 / +0x107c | ground-probe height walk / ball (B1:79608-79611) |
| 0x5b491acc / 0x829299ad ("1.0"/"10.0") | +0x2870 (1.0) / +0x2874 | +0xe80 / +0xe84 | fall-anim timer / ? |

No `RollingSound`/`BallSpeed`/`JetJump`/`JumpHeight` key exists in either droideka
loader [V]; the soldier's JumpHeight (0xc415d23a, re-stance-roll.md) is not read here.

## §2 The state machine

**States** (`+0x2040` BF1 / `+0x1ad8` BF2): 0 idle, 1–2 turn-in-place, 3 stop?, 4 dead,
5–10 walk-cycle phases, **0xb IdleToRoll, 0xc Rolling, 0xd RollToIdle** — anim ids per
state come from class+0x1860 (BF1) / class+0x758 (BF2) and the anim-name table
0x517780/0x5fcee8 ("TurnLeft … IdleToRoll, Rolling, RollToIdle") [V]. Naming of 0xb/0xc/0xd
is verified by the setter: entering 0xb stops the weapons (vtable +0x58 per weapon), leaving
0xd→0 restarts them (BF1 `FUN_001d0a68` B1:79063-79100; BF2 `FUN_001f76f0` B2:108219-108249),
and BF2 swaps the collision prim to `coll_ball` on entering 0xc (B2:108250-108255) [V].

**Transition table** (event 0..7 × state 0..0xd → next state): BF1 at 0x517588, used by
`FUN_001d0cc8` B1:79149 `FUN_001d0a68(this, tbl[state*8+event])`; BF2 at 0x5fccc8, used
by `FUN_001f7930` B2:108303. **Byte-identical tables** [V]:
```
state: 0   1   2   3   4   5   6   7   8   9   a   b   c   d      event
       0   0   0   4   4   9   a   a   9   0   0   c   d   0      0 stop
       0   0   0   4   4   0   0   5   7   5   7   c   d   0      1
       5   5   5   4   4   7   8   8   7   5   5   c   c   0      2 fwd
       9   9   a   4   4   a   9   8   7   8   7   c   c   0      3
       1   0   0   4   4   9   a   a   9   1   1   c   c   0      4 turn L
       2   0   0   4   4   9   a   a   9   2   2   c   c   0      5 turn R
       3   3   3   4   4   3   3   3   3   3   3   4   4   4      6 die
       b   b   b   b   b   b   b   b   b   b   b   c   c   0      7 ROLL
```
Event 7 from any walking state → 0xb; the anim-end of 0xb → event 0..5 → 0xc; event 0/1
while Rolling → 0xd; anything in 0xd → 0 [V].

**Who raises event 7 (transform).** BF1 `FUN_001cf770` (main vtable +0x108, called every
tick from the thread update B1:79725): human (`+0x388` ≥ 0) with command byte `+0x2f2 == 1`
(= Controllable byte +0x52): if state ∈ {0xb,0xc,0xd} → nothing; if Rolling and
`FUN_001d30d8` (headroom probe: capsule 1.1× tall clear, B1:80903-80925) → event 0
(unroll); else → event 7 (B1:78089-78144). AI: order `+0x40c` 1 = unroll, 2 = roll
(B1:78094, 78122). While in 0xb..0xd the fire bytes +0x2e8/+0x2e9 (ctrl +0x48/+0x49) are
zeroed → **cannot fire while rolling** (B1:78148-78150) [V]. The byte +0x52 is filled by
the pad→command mapper `FUN_0025e...` from logical button bit 7 (B1:180042-180093; bit 2 →
jump byte +0x4e, bit 3 → crouch +0x50) [V]; the physical PS2 button behind bit 7 is not
traced [H]. BF2 `FUN_001f6110` (main +0x128, B2:107196): human (`+0x330` ≥ 0) with
`(+0x2b8 & 3) == 3` (crouch/roll button pressed-edge, re-stance-roll §4.2): Rolling &&
`FUN_001fafd8` (grounded && headroom clear, B2:110875) → event 0; not in 0xb..0xd → event 7
(B2:107209, 107247-107250, 107310); AI via ctrl→+0x504 == 1/2; while 0xb..0xd
`+0x2a8 = (&~0x1f)|0xc` (fire bits forced off [D]) [V]. **BF2's transform button is the
crouch button; the jump button is free for the ball jump.**

**Transform durations.** BF2: anim playback rate of state 0xb = len/RollTime, 0xd =
len/UnrollTime (B2:113299-113301); ODF 0.50 s / 1.66 s [V]. BF1: no such keys; the states
last one anim (`FUN_001d0a68` sets +0x1bd4 from the anim length × class+0x26c0 × 30/fps,
B1:79053-79055) — the seconds live in the anim bank, not the ELF [V/H].

**Ball physics.** Both games run their own integrator, not the soldier's stance code:
- BF1 `FUN_001d0720` (main +0x114, dispatched when state == 0xc, B1:79830-79835): yaw rate
  = −turn·(class+0x26c4 + (BallTurnSpeed − class+0x26c4)·f(fwd))·dt applied to the matrix
  rows +0x200..+0x230; lean = normalize(1, turn·BallMaxLean); ω(+0x20d0) += dt·stick²·
  ±BallAcceleration, clamped to ±BallMoveSpeed·k(+0x2888); spin quaternion (+0x20c8/cc)
  integrated (B1:78842-78999) [V]. World velocity is written in **PostCollision**
  `FUN_001ce828` from the contact: v(+0x5c0) = ω·(−BallRadius/k)·lean-rotated contact
  frame when the contact slope exceeds MaxBallAngle (B1:77817-77838); the ball only moves while
  touching something [D]. Friction: ω −= BallRollingFriction·contact-weight (B1:77939-77956).
  **Bounce:** if v·n < 0 the velocity is replaced by the VU-computed reflection
  (`a local+a local` = 2(v·n), B1:77796-77808) — the only vertical impulse the BF1 ball ever
  gets, and a candidate for the remembered "hop" [H]. Gravity −9.8 dt in the thread
  update (B1:79941); position integrates the full velocity only in state 0xc, otherwise
  y only (B1:79914-79940) [V].
- BF2 `FUN_001f7110` (main +0x134, dispatched at B2:109511-109512): same shape — yaw rate
  lerp class+0xe90→BallTurnSpeed (B2:107948-107952), boost flag bit 5 of +0x1b10 selects
  class+0xe98; ω(+0x1b40) += dt·fwd·BallAcceleration(·class+0xeb8 = 2.0 when boosting)
  (B2:108101-108121); friction BallRollingFriction·dt when grounded (bit 6) (B2:108124-108138);
  clamp |ω| ≤ BallMoveSpeed·k/BallRadius (×2 boosting) (B2:108139-108149); spin
  (B2:108150-108160) [V]. Boost: `FUN_001f3508` sets bit 5 only in state 0xc and not
  energy-depleted (B2:105186-105195); energy drains `EnergyDrainSprint`·dt while bit 5 set,
  otherwise restores (`FUN_001f8548` B2:109520-109541) [V].

**The JUMP.**
- **BF2 [V]:** `FUN_001f7110` B2:108096-108098: `if ((this+0x2b4 & 3) == 3) { ctrl
  vtable+0xb8 (boost off); ctrl vtable+0xac (JUMP) }` — +0x2b4 are the same jump bits the
  soldier reads (re-stance-roll §4.2, "pressed edge = (bits&3)==3"). +0xac →
  `FUN_001ff238` → `FUN_001f3360` (B2:105115-105165): requires **state == 0xc** and
  **airborne timer +0x1b74 < 0.1 s** (`DAT_005fce88` = 0.1; the timer += dt while bit 6
  (grounded) is clear and resets to 0 on ground, B2:109492-109509) and
  `FUN_001e2010(EnergyCostJump, energy+0x1b78)` (spend; fails only when the pool is
  flagged depleted, B2:92619-92635). Then: timer = 0.1 (no re-jump until landing), plays
  `JumpSound` (class+0x1064, B2:105141), **vel.y += class+0xebc** (B2:105142), and if
  vel·facing > 0: vel += facing·(vel·facing)·(class+0xec0 − 1) (B2:105143-105157) [V].
  Numbers (PS2 BF2 ODF, `bf2_cis.ron` cis_inf_droideka): BallJumpHeight 1.27 → v0 =
  4.99 m/s; 0x388d3934 = 1.25 → +25 % forward; EnergyCostJump 30, EnergyBar 100,
  EnergyRestore 10/s (15 idle) [V]. Engine defaults if unset: v0 5.0 m/s, factor 1.25,
  cost 30 (class ctor B2:112038-112039, 112056) [V]. **So BF2's ball jumps ~1.27 m (up to 3× on a
  full bar) — the opposite of the premise.** Jump in walk form: `FUN_001f3360` returns 0
  unless state == 0xc, and the walk path calls only boost-off (B2:109516) → **BF2's
  droideka cannot jump while walking**, only as a ball [V].
- **BF1 [V, by exhaustion of the region B1:76381-83385 = 0x1cd060-0x1d6f90]:** the
  Controllable jump slot is the `jr ra` stub (§1); no function in the region reads the
  jump byte +0x2ee/ctrl+0x4e (grep, none); the only writers of velocity.y (+0x5c4 /
  [0x171]) are zeroing at B1:77692, 79134 and gravity at B1:79941, 80317; no key resembling
  a jump height exists in the loader or in CIS.LVL. **BF1's droideka cannot jump, in ball
  or walk form.** The thread update (`FUN_001d1540`, profiler tag "DroidekaUpdate"
  0x584720, B1:79562) is the entire per-tick path: ground probe → optional fall anim →
  weapon-slot switching → +0x108 (`FUN_001cf770`, transform events) → +0x10c
  (`FUN_001cf8d0`, weapon fire/aim) → +0x110/+0x114 motion → integrate → gravity [V].

## §3 Soldier variant or vehicle?

Neither. In both games EntityDroideka is a peer of EntitySoldier under the shared
"controllable entity" base (same class-base ctor, same Controllable sub-object and input
bytes, same PostCollision/Thread mixins, same AI kind table — RE_LOG: `FUN_00234c00`
EntityDroideka = kind 3 vs EntitySoldier 0, EntityWalker 2). It does not use the soldier
stance machine (`soldier+0xbe4` / `+0x7e4`), the soldier physics integrator, or the
vehicle/walker classes (`walker` → `FUN_0021cca8` / `FUN_00263160` are separate). For the
port: build it as a **soldier-adjacent unit class** (reuses input layer, capsule/ground
probe, weapons, energy pool in BF2 terms) with its own 14-state table above — not P7
(vehicles).

## §4 ODF properties, BF1 CIS.LVL vs BF2 (droideka-specific keys only)

BF1 from `extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL` entc `cis_inf_droideka`
(BASE walkerdroid), BF2 from `engine/assets_baked/classes/bf2_cis.ron` [V].

| key | BF1 | BF2 |
|---|---|---|
| MaxSpeed | 5.0 | 2.5 |
| MaxTurnSpeed | 1.25 | 1.25 |
| BallMoveSpeed | 10.0 | 6.0 |
| BallRadius | 0.65 | 0.65 |
| 0xdc524dd9 (ω units factor) | 2.0 | 1.0 |
| BallTurnSpeed | 2.0 | 2.0 |
| BallAcceleration | 100.0 | 100.0 |
| BallRollingFriction | 50.0 | 50.0 |
| BallMaxLean | 10.0 | 10.0 |
| MaxBallAngle | 35.0 | 35.0 |
| BallMoveThreshold | 0.2 | — |
| 0xed895ee0 | 0.1 | — |
| BallCollision | p_rollsphere | — (BF2: TerrainCollision→coll_ball swap, class+0xefc) |
| **BallJumpHeight** | **—** | **1.27** |
| **0x388d3934 (jump fwd factor)** | — | 1.25 |
| **EnergyCostJump** | — | 30.0 |
| EnergyBar / Restore / RestoreIdle | — | 100 / 10 / 15 |
| EnergyDrainSprint / EnergyMinSprint | — | 30 / 30 |
| RollTime / UnrollTime | — | 0.50 / 1.66 |
| 0xf01d5583 / 0xac229931 (probe heights) | 1.5 / 0.5 | 1.5 / 0.5 |
| 0x5b491acc / 0x829299ad | 1.0 / 10.0 | 1.0 / 10.0 |
| TurnThreshold | 0.50 | 0.50 |
| 0x7c8f02c0 / ForwardTurnSpeed | 0.8 / 0.8 | — |
| MaxHealth | 450 | 550 |
| Weapons | cis_weap_inf_repeater, cis_weap_walk_droideka_shield | same |

BF1 top ball speed 6.5 m/s vs BF2 6.0 m/s [D]; BF1 walk 5.0 vs BF2 2.5 m/s [V].

## §Reproduction

```
cd <repo>; S=engine/artifacts/lead_tmp/s12/re/droideka_scratch
grep -n "walkerdroid" analysis/SWBFRONT.ELF-stringrefs.txt analysis/SWBFRNT2.ELF-stringrefs.txt
sed -n 15588,15590p analysis/SWBFRONT.ELF-decomp.c; sed -n 16399,16401p analysis/SWBFRNT2.ELF-decomp.c
python3 $S/vtdump.py extracted/bf1/SWBFRONT.ELF 52ffc8 40 analysis/SWBFRONT.ELF-functions.txt   # droideka ctrl vt; +0x80 = 0023e8f0
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 23e8e8 12   # jr ra stubs
python3 $S/vtdump.py extracted/bf2/SWBFRNT2.ELF 61ba60 52 analysis/SWBFRNT2.ELF-functions.txt   # +0xac = 001ff238
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 517588 112   # BF1 transition table
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 5fccc8 112   # BF2 (identical)
sed -n 105115,105165p analysis/SWBFRNT2.ELF-decomp.c   # BF2 ball jump; 108096,108098 its call
sed -n 78078,78158p analysis/SWBFRONT.ELF-decomp.c     # BF1 transform events
sed -n 112458,113211p analysis/SWBFRNT2.ELF-decomp.c | awk -f $S/loadmap.awk | sort   # BF2 hash->field
sed -n 81430,82078p analysis/SWBFRONT.ELF-decomp.c | awk -f $S/loadmap.awk | sort     # BF1 hash->field
python3 $S/brute2.py $S/bf1_map.txt $S/bf2_map.txt $S/ron_keys.txt                    # preimages
python3 $S/lvlprops.py extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL cis_inf_droideka     # BF1 ODF
awk 'NR>=76381&&NR<=83385&&/\[0x171\] = |0x5c4\) = |0x2ee/' analysis/SWBFRONT.ELF-decomp.c  # BF1 vel.y writers
```
Scratch files: `$S/bf1_vt.txt bf2_vt.txt bf1_map.txt bf2_map.txt brute2_out.txt
bf1_droideka_props.txt`. No Ghidra runs were needed.

## §Not closed

- Physical PS2 button behind BF1 logical bit 7 (ctrl byte +0x52; also read by the flyer
  B1:85473 and soldier B1:129506) — needs the pad→logical map or a PCSX2 check [H].
- Whether BF2 PS2 actually delivers the jump edge (+0x2b4) to a droideka-controlled
  Controllable was not traced through the input mapper; the read is the same field the
  soldier uses, so I expect yes — a PCSX2 check would settle the maintainer's memory [H].
- BF1 ball world-velocity formula is read from VU-heavy PostCollision code; the reflection
  (bounce) and v = ω·r/k are [D], magnitudes not measured.
- Unresolved hashes: 0xdc524dd9, 0x388d3934, 0x7c8f02c0, 0x12a7e73d, 0xed895ee0,
  0xf01d5583, 0xac229931, 0x5b491acc, 0x829299ad, 0x3437a33d (BF1 "3.0"), 0xbebb6ed6
  (BF2 boost mult 2.0), 0x4a6ac53f (+0xeb4, 8.0), 0x81d52461 (+0xeb0, 0.5).
- BF1 transform durations (anim lengths in the CIS.LVL zaa_ bank) not extracted.
