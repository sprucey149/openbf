# OPUS RE (2026-10-03) - [O] UNAUDITED

# RE: the droideka as a fielded unit - shield, walk form, collision, AI, measured motion (card DEKA)

Extends `re-droideka.md` (the class, the 14-state table, the ball integrator, BF2's ball jump) and CORRECTS it in
three places (section 0). Corpora: B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c`; the discs' `CIS.LVL`;
PS2 RAM (450 droideka rows out of 219 stored save states; two live PCSX2 samples, 139 s BF1 and 110 s BF2, timed by
the game clock). Tags: [V] read at the cited lines, [M] measured on the PS2, [D] derived, [H] hypothesis; `[O]` marks
what the build cards rest on. E = the entity (BF1 vtable 0x530138 at E+0, BF2 0x61bc50); comp = its health component
(BF1 E+8: health, max, rate, shield, shield max, shield rate, flags at E+0xac; BF2 E+0x170.., flags E+0x228).

Hops: label tables -> WeaponShield class / instance (both games) -> health component update and TakeDamage -> the bolt
hit; EntityDroideka thread update -> walk function -> PostCollision; the AI's form order from its ctor default to every
writer; the combat helper's shield rule. Two Opus readers (`lead_tmp/deka/AGENT_WALK.md`, `AGENT_AI.md`), their
load-bearing lines re-read by the lead; one Opus verifier on 37 claims given without reasoning
(`lead_tmp/deka/CLAIMS.md` -> `verify/VERDICTS.md`): **28 SUPPORTED, 9 PARTLY, 0 NOT SUPPORTED**, every PARTLY folded
in below. Checks run: `re_check.py fn` on every cited line, `refs.py` for vtable-only callers, ELF store scans for the
order field, `re_check.py word` for constants, disassembly of the holes (0x294080, 0x2ac4d0, 0x1d1c18).

## 0. Headline

1. **[M][O] The ball moves at TWICE its stored velocity, in both games.** The entity runs two thread updates per
   tick and each integrates position and gravity (7.1). Live: position speed = 1.999 x the stored XZ velocity (BF1,
   5,238 tick pairs), top 12.96 m/s (BF1; stored 6.50 = omega 20 x 0.65 / 2) and 11.99 m/s (BF2; stored 6.0). Bots
   cruise at 10.2 m/s (BF1 median) and 11.9 m/s (BF2). `re-droideka.md`'s "6.5 / 6.0 m/s" is the stored value.
   The fall is doubled too: stored vy changes by 19.6 /s and moves at 2 vy, so a droideka falls at 39.2 m/s^2.
2. **[M][O] Fold 0.50 s, unfold 1.67 s in BOTH games** (BF1 live: 4 of 5 and 13 of 13 runs; BF2 1.635 s). BF1 gets
   there by playing every clip at rate 2.0 (key 0x6b8d8679); BF2 by `RollTime` 0.50 / `UnrollTime` 1.66.
3. **[V][M][O] The shield** is 3000 points of its own, drained 100 /s while up and refilled 300 /s while down
   (`AddShield` -100, `AddShieldOff` 300; live -100.0 / +300.0). A press toggles it; it is forced down in the fold /
   ball / unfold states and at 0. Up, it is a sphere r 1.25 at (0, 1.0, 0.125) that takes a bolt's damage x
   `ShieldScale` and passes none on; a bolt that reaches the body bypasses it (section 2).
4. **[V][M][O] The AI** rolls everywhere and deploys only for a fight or to walk out of a stuck spot: the form order
   defaults to "roll", the combat helper writes "deploy" on enter and every update and "roll" on exit (section 5).
   In RAM: 218 of 218 ball rows hold order 2, 189 of 191 walk rows order 1, none 0. Deployed in the attack state with
   the shield down and over half: it raises it. It never lowers it. It starts a fight from sight only within 10 m.
5. **[V][M] The walk form** is not a velocity: BF1 moves by the walk clip's root motion x 2 (2.26 m/s measured in the
   loop; `MaxSpeed` 5.0 only feeds the AI), BF2 scales the clip so the loop moves at `MaxSpeed` 2.5 m/s. BF1 cannot
   strafe, BF2 can; both walk backwards; neither accelerates (`Acceleraton` has no reader).
6. Corrections to `re-droideka.md`: (a) ball speed, item 1; (b) there is NO ground probe - keys 0xf01d5583 /
   0xac229931 (`BallWaterDamageHeight`) are water depths and the "fall timer" is the water-damage timer (7.3);
   (c) main vtable +0x10c (BF1) / +0x12c (BF2) is the human's camera / zoom, not weapon fire.

## 1. The shield weapon (class label `shield`, WeaponShield) [V][O]

| key (fnv) | BF1 class | BF2 class | disc (both) |
|---|---|---|---|
| MaxShield 0xda3491bc | +0x1e0 | +0x240 | 3000 |
| AddShield 0xbad94c15 (rate while up) | +0x1e4 | +0x244 | -100 |
| AddShieldOff 0x729dfa7a (rate while down) | +0x1e8 | +0x248 | 300 |
| ShieldOffset 0x1d726313 | +0x1f0 | +0x250 | 0.0 1.0 0.125 |
| ShieldRadius 0x5a9201b6 | +0x200 | +0x260 | 1.25 |
| ShieldSound 0xd85503ff / ShieldOffSound 0x5df6c1a4 / ShieldEffect 0xddb924a1 | +0x2cc / +0x2d4 / +0x2c8 | +0x3c4 / +0x3cc / +0x3c0 | BF1 `droid_fx7_hum` / - / `droidekashield`; BF2 - / - / `droidekashield` |

Names `AddShieldOff`, `ShieldRadius`, `ShieldSound`, `ShieldOffSound`, `ShieldEffect` found by hash this pass. Label
B1:15483-15484 -> class ctor `FUN_00294178` (vtable 0x53ca18), setter `FUN_00294428` B1:219574-219662; B2:16299-16300
-> `FUN_00349108` (vtable 0x62a048), setter `FUN_00349398` B2:341149-341237.

- **Spawn** (instance ctor `FUN_00293660` B1:219029-219095; `FUN_003484d8` B2:340557-340631): a collision body in the
  shield layer (0x800 BF1, 0x1000 BF2), radius = the largest `ShieldRadius` float; comp shield max = MaxShield; shield
  = MaxShield because the slot's ammo is infinite (`WeaponAmmo 0`; else 0); rate = AddShieldOff; the shield-on flag
  (BF1 bit 4 of E+0xac, BF2 bit 5 of E+0x228) clear. So it spawns full and down.
- **Each tick** (weapon update `FUN_00293920` B1:219120-219310; `FUN_00348818` B2:340656-340853): on = the flag;
  shield <= 0 -> off; the alive bit (bit 3) clear -> off; owner an EntityDroideka in state 0xb..0xd -> off; else a
  PRESS EDGE of the slot's trigger toggles it (BF1 the frame-count byte == 1; BF2 `(bits & 3) == 3`). Then rate = on ?
  AddShield : AddShieldOff. While on: the body is in the world at owner matrix x ShieldOffset, the `ShieldEffect`
  object is alive, `ShieldSound` loops with gain x (1 - (1 - shield / max)^2); going off plays `ShieldOffSound`.
- **No refill on a press**: the weapon's "wants to fire" predicate returns 0 for infinite ammo (BF1 0x294080, a hole
  in the decomp, disassembled: ammo == FLT_MAX at 0x53c9f4 -> 0; BF2 `FUN_00348fe8`), so the fire-state function that
  sets shield = max (`FUN_00293f98` B1:219354-219380, `FUN_00348ef0`) never runs for the droideka.
- **Integration** (comp update): BF1 `FUN_001bbbe0` B1:64092-64125: shield += dt x rate, floor 0, cap max. BF2
  `FUN_001df560` B2:91036-91075 does that ONLY when shield < max (91059-91060): **a BF2 shield raised at exactly
  3000 does not drain until a hit takes it under max** [V][M: BF2 live 69 consecutive ticks up at 3000.0, 3 of 9
  shield-up rows in the stored states; BF1 0 of 137 rows, 9 single-tick rows live].
- **Times** [D]: 30 s from full to empty while up and unhit (BF1); 10 s from empty to full while down.

## 2. Damage against the shield [V][O]

- TakeDamage, shield phase (BF1 `FUN_001bc190` B1:64364-64379; BF2 `FUN_001dfb38`): only when the caller's flags have
  bit 0, shield > 0 and the shield is on: sd = dmg x the ordnance's `ShieldScale`; sd <= shield -> shield -= sd,
  dmg = 0; else dmg -= shield / ShieldScale, shield = 0. The health phase runs only with flag bit 1. BF2 adds a
  floor for a non-player attacker: shield and health do not fall under comp[7] x max (B2:91346-91356; comp[7] is 0
  unless the script calls `SetAIDamageThreshold`).
- The bolt's hit (BF1 `FUN_00276810` B1:197797-197833; BF2 `FUN_00313c40` B2:304534-304584) picks the flags: the
  struck body is in the shield layer -> flags 1 (shield phase ONLY: a bolt that empties the shield does not reach the
  health) and the victim is the shield's owner; the struck body has an attached shield body whose radius is over 1.5 x
  its own -> flags 2 (health ONLY); else both. BF1's bolts meet `p_rollsphere` (r 0.65; 0.975 < 1.25) and BF2's the
  `p_-o-` boxes, so **a bolt that reaches the droideka's body skips the shield entirely** - a shot from inside the
  1.25 m sphere. Other damage (water, crash, kill floor) passes flags -1 = both phases (`FUN_001bc8e0`).
- Not opened [H]: which flags an explosion passes; BF1's shield layer is in the blast's gather mask (re-vehicle-death).
- The repeater's own bolt: BF1 `Damage` 55, `Velocity` 80, `LifeSpan` 1.5; BF2 80 / 160 / 0.8; `ShieldScale` 1.0.

## 3. The repeater (`cis_weap_inf_repeater`, BASE `cannon`) [V disc]

An ordinary cannon the port's `WeaponClass::from_props` already parses: BF1 `ShotDelay` 0.11, `HeatPerShot` 0.089,
`HeatRecoverRate` 0.5, `HeatThreshold` 0.20, `RoundsPerClip` 0 (no clip); BF2 `ShotDelay` 0.125, `HeatPerShot` 0.09.
Spread keys `SpreadRecoverRate` 4.8, `SpreadThreshold` 2.0, `SpreadLimit` 6.0; ranges 0 / 24 / 64. The class section
gives it two aimers (`hp_gun_1`, `NEXTAIMER`, `hp_gun_2`; aimer limits yaw +-40, pitch +-25) - the aimer objects were
not opened [H: shots alternate between the two fire points]. No firing in states 0xb..0xd (5.1).

### Bot shot parallax (issue #108)

The body remains upright on a slope; pitch is measured from world level and
clamped to the class limits (BF1 `FUN_001d1540`, B1:79905-79912,
`FUN_001d1fe8`, B1:80030-80049; BF2 `FUN_001f8548`, B2:109617-109624).
Deploying during a fight does not test slope or line of fire (BF1
`FUN_001cf770`, B1:78076-78153; BF2 `FUN_001f6110`, B2:107194-107285).
Keep that deployment behavior.

The port now converges bot bolts from the gun hardpoint to the current aim
line at the perceived contact's range, bounded by `AimerPitchLimits` and
`AimerYawLimits`. It retains tracking error rather than pointing straight at
the enemy's centre. Human shots retain their existing reticle convergence.
This bot barrel rule is **PROVISIONAL**, trace: PCSX2 BF1/BF2 barrel spawn
position and velocity on a ramp, including both `hp_gun_1/2`, alternation,
convergence versus parallel fire and behavior beyond the pitch limit. The
PS2 aimer-to-bolt path remains untraced; the synthetic ramp regression is
port correctness evidence, not original-game acceptance.

## 4. Forms, clips and times [V][M][O]

State names are the ELF's (table 0x517750 BF1): 0 Idle, 1 TurnLeft, 2 TurnRight, 3 Dying, 4 Dead, 5 IdleToLeft,
6 IdleToRight (no clip, never entered), 7 LeftToRight, 8 RightToLeft, 9 LeftToIdle, 10 RightToIdle, 11 IdleToRoll,
12 Rolling, 13 RollToIdle. Clips of the bank `cis_walk_droideka` (class `FUN_001d50e8` B1:82088-82160; names are the
bare strings below, stored as the complemented CRC-32/MPEG-2 of the lowercased name; both discs' banks hold the same
frame counts, BF1's has three more unused clips):

| state | clip | frames | PS2 time (BF1 live) |
|---|---|---|---|
| 0 | `idle` | 46 | - |
| 1 / 2 | `turnl` / `turnr` (30 degrees of leg turn per play) | 9 | 0.133 s |
| 3 | `death01` | 81 | one tick (7.4) |
| 5 | `idle_to_leftfoot` | 17 | 0.267 s |
| 7 / 8 | `walk_leftfoot_rightfoot` / `walk_rightfoot_leftfoot` | 17 | 0.267 - 0.300 s |
| 9 / 10 | `leftfoot_to_idle` / `rightfoot_to_idle` | 24 | 0.400 s |
| 11 | `fold` | 31 | **0.500 s** |
| 12 | `roll` | 11 | loop |
| 13 | `unfold` | 101 | **1.668 s** |

BF1: time = (frames - 1) / 30 / 2.0 - the class setter of key 0x6b8d8679 (disc 2.0; class+0x26c0) divides every state
duration by it and the walk function advances every clip at that rate (B1:81743-81755, 78498-78507). BF2 ignores that
key and sets per-state rates: fold = `RollTime`, unfold = `UnrollTime`, walk states 5 / 7 / 8 / 9 / 10 at
MaxSpeed / (state 7's natural speed), turns at 0.51 x MaxTurnSpeed / 30 degrees (`FUN_001fe818` B2:113289-113297).
Five bones are looked up by name: `DummyRoot`, `center_box`, `bone_spine1..3`.

## 5. The AI [V][M][O] (rows 1-37 of `lead_tmp/deka/AGENT_AI.md`; both games unless said)

1. **The form order** lives on the mover (BF2 ai+0x504 = mover+0xd4; BF1 Controllable+0x16c = E+0x40c): 1 = deploy,
   2 = roll. Default 2 from the ctor and every Controllable reset (BF2 `FUN_002ba3c8` B2:241466, `FUN_002ba510` 241506,
   `FUN_002ba720` 241576; BF1 `FUN_0023aa88` B1:153543, `FUN_0023a818` 153423). Writers of 1: the combat helper's
   enter and EVERY update (BF2 `FUN_002ce8b8` B2:256176, `FUN_002cf458` 256626; BF1 UnitCombatAgent `FUN_002450b8`
   B1:161231, `FUN_002456e0` 161445) and UnitRandomAgent's update (the stuck walk: B2:253671, B1:163962). Writers of
   2: the helper's exit (B2:256365, B1:161405) and the random agent's exit (B2:253641, B1:163928). No other AI writer
   (ELF scan of every store at the offset; the rest are network restores B2:471321, 476266, B1:318974 and a dead stub).
   The entity acts on it in `FUN_001cf770` B1:78076-78153 / `FUN_001f6110` B2:107194-107285: Rolling + order 1 +
   floor and headroom (capsule 1.1 x) -> event 0 (unroll); a walk state + order 2 + alive -> event 7 (roll). In
   states 0xb..0xd the fire inputs are cleared (B1:78147-78150; B2:107281-107282).
   **In words:** a bot droideka rolls up the moment it spawns and travels as a ball through its whole capture / hold /
   follow order; it deploys for the whole life of a fight order (go to the point, look around, go to the noise, attack,
   evade) and rolls again when the fight ends; stuck (no 2 m in 6 s, or no progress for 6 s) it unrolls, WALKS to a
   random point within 10 m, is done 2 m out, and rolls.
2. **The shield** (BF2 `FUN_002cf458` B2:256907-256916; BF1 `FUN_002456e0` B1:161509-161534): in the attack state,
   after the fire sub-helper ticked: shield flag off AND shield > 0.5 x max -> select weapon slot 1, force fire, skip
   this think's fire request. The repeater is silent for that one think; the next think's weapon pick takes slot 0
   back (the shield weapon has no ordnance and never scores). No AI code lowers the shield, and at or under half it is
   not raised. It drops by itself at the roll (1) or at 0. [M] BF1 stored states: 137 of 174 deployed rows have it
   up, 52 of them under half (kept until empty); the 37 down rows include 10 at exactly 3000 ([D] deployed, but not
   in the attack state).
3. **Targets**: the "enemy seen" candidate gives a droideka no fight order beyond 10 m (3-D; BF2 message 5
   `FUN_002c31e0` B2:248138-248159; BF1 `FUN_00252ae8` B1:171314-171331, the third of eight candidates of
   `FUN_00253cd0`). Farther enemies reach it through damage or noise, both gated by the attack matrix. The matrix for a
   droideka attacker: mode 2 (stand and shoot: no path, no strafe, no advance) when a weapon is chosen and in range,
   else rejected -> look around -> fight over -> roll; against a flyer always rejected (BF1 `FUN_00239400`
   B1:152412-152425, `FUN_002393c8`; BF2 0x2ac4d0 disassembled, `FUN_002ac490`). It never gets the soldier's
   dash-away or passing-shot evade.
4. **Steering input** (BF2 `FUN_001fa4b0` B2:110212-110252; BF1 `FUN_001d2a08` B1:80507-80555): forward speed =
   walk `MaxSpeed`, strafe speed 0, turn rate `MaxTurnSpeed` 1.25 rad/s (`BallTurnSpeed` only under a flag that the
   soldier's input alone writes [D]). Arrival radius 4 m (soldier 0.5 / 1.0), full stop inside 0.2 m; hub-point margin
   4.0 m; stuck thresholds 2 m / 6 s / 6 s; hold radius 2 m (RE_LOG).
5. **Not for a droideka**: vehicles and turrets (the class's enter flag defaults to 0 for a walkerdroid, 1 for a
   soldier: B1:121820 vs 81014, B2:169394 vs 111903; the board acceptance also needs EntitySoldier); item drops
   (re-powerups 3.1). It does capture posts and takes follow / hold orders; a follower whose leader boards is released.
6. BF2 only, a droideka as TARGET: the 22.5 degree aim tolerance, the grenade's 1-in-10 window and the weapon's
   AITarget mask have droideka exceptions (AGENT_AI rows 23, 25). Melee scores 1000 against it in both.
7. [M] BF1 deployed bots are almost never in a walk-cycle state (0 of 360 stored rows; 125 tick pairs = about 4 s
   in 139 s live, four droidekas); BF2 4 of 21 stored rows.

## 6. The player's droideka [V unless said]

- Fold / unfold: BF1 the action byte E+0x2f2 == 1 (logical button 7, pad button [H]); BF2 the crouch button's press
  edge. Unfold needs the floor (BF2) and headroom. Shield: the trigger of weapon slot 2's channel, press edge.
- Walk form: the stick yaws the BODY at once; legs lag and catch up by 30 degree turn-in-place clips once the leg
  offset passes `TurnThreshold` (BF1 0.5 rad, BF2 30 degrees). Yaw rate for a human = MaxPitchSpeed + (MaxTurnSpeed x
  sensitivity - MaxPitchSpeed) x the aim factor, / the zoom factor (B1:78397-78438; B2:107472-107484). The aim is the
  body's forward tilted by a pitch clamped to `PitchLimits` +-25 degrees (B1:79906-79912, 80030-80049); class
  `YawLimits` has no reader. BF1: no strafe, `turn = turn x (1 - |strafe|) + strafe` (B1:79802-79804). BF2: the legs
  steer to the stick direction at MaxTurnSpeed while the body keeps its aim (B2:107492-107538). Both: back = the walk
  clips in reverse (event 3; B1:79118-79133, B2:108272-108287).
- Ball form, the jump (BF2 only), energy: `re-droideka.md` sections 2 and 4, with the speeds of 7.1.

## 7. Motion, collision, water, death

### 7.1 Two integrations a tick [V code][M effect][O]
The thread update integrates (ball: position += velocity x dt; other states: y += vy x dt; then vy -= 9.8 x dt:
`FUN_001d1540` B1:79913-79941, `FUN_001f8548` B2:109626-109653) and the SAME block stands in the update of a second
thread of the same entity (main vtable +0x104 `FUN_001d1fe8` B1:80289-80317; +0x124 `FUN_001f99d0` B2:110005-110033;
both nodes registered by `FUN_001d2ff0` B1:80870-80871, each called once per tick with the same dt by `FUN_00197350`;
no guard found by the reader or the verifier). Live: ratio 1.999 (p10 1.999, p90 2.000), free-fall d(vy)/dt = -19.60,
while the shield slope is exactly -100 /s (the comp runs once). The walk form's XZ motion is inside the walk function
and is NOT doubled. Port numbers: ball ground speed = 2 x omega x r / k -> BF1 13.0 m/s top (BallMoveSpeed 10,
BallRadius 0.65, k 2.0), BF2 12.0 m/s (the player's boost doubles the clamp again, re-droideka 2: [D] 24 m/s, not
measured); fall acceleration 39.2 m/s^2 on a displayed
vertical speed of 2 vy; BF2's jump: stored +4.99 m/s -> leaves at 9.98 m/s, apex 1.27 m (= `BallJumpHeight`), 0.51 s
in the air.

### 7.2 Collision shapes [V disc + code]
The entity is in the SOLDIER layer (ctor passes 2). Bodies of `cis_walk_droideka` (`lead_tmp/deka/tools/deka_prims.py`):
- BF1 (class keys pick the subsets): terrain = `p_rollsphere` (sphere r 0.65 at y 0.65) + `p_base` (cylinder r 1.75,
  h 2.0, at y 1.0); soldier = `p_base`; building = `p_rollsphere` + a body made by `BuildingCollisionPrim`
  0x8cf29543 ("BldgSphere sphere NULL 0.0 1.0 0.125 1.25": sphere r 1.25 at (0, 1.0, 0.125), building subset only;
  base setter `FUN_001e3678` B1:93161-93330); ordnance and vehicle = `p_rollsphere`. In ball form only contacts of the
  `BallCollision` body (`p_rollsphere`) are answered and soldiers do not push it (B1:77469-77472, 77746-77759).
- BF2 (MASK bytes on the prims): 13 `p_-o-` boxes / cylinder / sphere on the bones (ordnance), `p_-bt-building` sphere
  r 1.0 at (0, 0.992, 0.421) (buildings + terrain), `p_-s-yoda` cylinder r 1.0 h 2.0 (soldiers); the ball swaps the
  whole model for `coll_ball`, a sphere of BallRadius at (0, BallRadius + 0.1, -0.2) in all five subsets
  (`FUN_001fe818` B2:113232-113288; swap B2:108246-108252).
- Walk-form answer to vehicle / building / terrain (`FUN_001ce828` B1:77657-77745; `FUN_001f4d30` B2:106909-107035):
  n.y > 0.7 -> vy = 0, up by depth / n.y; 0 < n.y <= 0.7 -> out horizontally; else along the normal; position only
  (it slides, the next tick walks again). Against soldiers: the base push shared by bounding-box volume. Crash damage
  above a closing speed of 7.5 + 2.5 (up . n), squared (B1:77514-77648) - defaults, not measured.

### 7.3 Water, the kill plane, health [V]
Water surface >= origin.y + H (H = 1.5 walking, ball 0.5 BF1 / 1.5 BF2; keys 0xf01d5583 / `BallWaterDamageHeight`):
a timer runs down from `WaterDamageInterval` 1.0 and is not reloaded while submerged, so `WaterDamageAmount` 10 is
dealt EVERY update after the first second (B1:79608-79646; B2:109306-109345). y < -500 or under the world floor kills.
No regeneration (neither disc authors `AddHealth`); `MaxHealth` 450 BF1 / 550 BF2, `HealthType` droid.

### 7.4 Death [V][M]
Alive bit clear: once - the dead flag, the collision object removed, weapons killed, `ExplosionName` spawned
(`FUN_001cf6a0` B1:78034-78072; `FUN_001f5fe8` B2:107137-107190); then event 6 every tick: walk state -> Dying (one
tick, live 7 of 7) -> Dead, which spawns the explosion again, the chunks (5 `CHUNKSECTION`s in BF1's class) and kills
the entity. `death01` never plays out and `DeathDustDelay` 5.1 is never reached offline [D]. The transition table
(0x517588 / 0x5fccc8) is 14 states x 8 events, state-major; row 3 is 4 4 4 4 4 4 4 11.

## 8. What stayed open

- [H] the explosion's damage flags against a raised shield; the two aimers' alternation; BF1's pad button for fold.
- [H] whether a soldier's dodge-roll request in the evade state does anything for a droideka (reader: it stands 2 s).
- Not opened: the attack sub-helper's cover check for mode 2; the chunk launcher; first-person (`droidekafp`, 4 clips).
- BF1's walk speed is measured (2.26 m/s in states 7 / 8, 30 + 16 tick pairs), not decoded from `DummyRoot`'s track.
- BF2 walk speed 2.5 m/s is by the rate table [V], not measured (no walking droideka in the BF2 sample).

## 9. Reproduce

```
T=engine/artifacts/lead_tmp/deka/tools
python3 $T/deka_ram.py $(find engine/artifacts -name '*.p2s' -size +1M)   # -> ../deka_ram_all.txt, the totals at its end
$T/deka_run.sh bf1 bf1_kas2_cw_cis_cp4_cp11_dock_two_spawn.p2s 150 bf1_kas2   # SW renderer, a PCSX2 slot, no ci.lock
$T/deka_run.sh bf2 bf2_kas2_cw_rep_cp0_cp4con_post3_spawn.p2s 180 bf2_kas2
python3 $T/deka_ana.py engine/artifacts/lead_tmp/deka/live/bf1_kas2.csv       # fold / unfold, ball speed ratio, shield slopes
python3 $T/deka_prims.py extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL droideka
python3 engine/artifacts/lead_tmp/veh0/entc.py extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/CIS.LVL cis_weap_walk_droideka_shield
python3 scripts/tools/eedis.py bf1 294080 2940d8     # the shield's fire predicate
```
The two stored CSVs were taken with BF2's velocity read at E+0x550; the real field is E+0x490 (verifier; the script
is fixed since) - no BF2 number above uses that column.

## 9. Remaining player feedback, chunks and feet (issue #85, 2026-11-01)

The ball sprint (the maintainer, 2026-11-01, a deliberate port choice replacing the 2026-10-30 "no sprint" rule):
a player's droideka sprints in ball form only, and BF2's rolling boost IS that sprint, so there is one mechanic.
From the disc (`re-droideka.md` §4): multiplier 2, forward stick over 0.9, the slower boost turn, `EnergyDrainSprint`.
OURS: it spends the one stamina bar, the shield's charge, not the disc's hidden jump pool, at the class's share of the bar
(`EnergyDrainSprint` / `EnergyBar`: 30 %/s, 900 points/s, 3.3 s from full; its recharge is `AddShieldOff`, paused
while sprinting); the button is held like a soldier's, not latched as on the disc; a bar that reaches empty, by sprint
or by the shield, locks the sprint until it is back at the soldier's recover share (25 %, 750 points, 2.5 s). These
values are PROVISIONAL: a maintainer's timed full-bar ball sprint and empty-to-next-sprint in live play replace them.
Transitions cannot refill anything: the one pool and the lockout carry through a fold and an unfold unchanged, only
ordinary recharge moves them. Unrolling ends the sprint on the press; a sprint button held through a fold starts once
the ball is formed, if the bar allows. A bar spent to empty and unrolled gives the shield only what recharged. Bots
never sprint and their hashes do not move. Measured in the sim, not on a disc: 24 m/s, 79 m per full bar.

The ball jump (the maintainer, 2026-11-02): like a soldier's, whose jump and sprint share one stamina bar, it spends
the same bar as the ball's sprint, the shield's charge, and the disc's separate hidden pool (`EnergyRestore`,
`EnergyRestoreIdle`) is gone from the port. The soldier's rule (BF2 `FUN_001e2010`, `core::soldier`'s `pay_jump`) at
the class's own share of the bar: `EnergyCostJump` / `EnergyBar` (BF2 30 of 100: 900 of 3000 points), the dearer
`EnergyCostJumpSprint` while sprinting (no droideka class authors it). A bar that cannot cover the cost is emptied
and the jump still happens; only a spent bar (emptied and not yet back at the 25 % recover share, the sprint's
lockout) refuses, so a refusal always shows on the bar. The stamina cheat makes it free. OURS and PROVISIONAL,
measure: the maintainer's ball jumps from a full and a low bar in live play. A refused grounded ball jump emits
"Shield charge spent — wait for recharge" for its player, a named UI addition. Bots never jump, so their play does not
change; the droideka's hash record lost the pool's two fields.
The aimed-at unit's existing shield HUD field now shows charge while its sphere is raised.

Droideka `CHUNKSECTION` geometry was omitted from the side vehicle bake. Both CIS classes already
author five sections (chunk1, chunk3 twice, chunk4, chunk5); include their models, collision and
hardpoints through the existing vehicle path. The duplicated chunk3 is native, not a missing chunk2.

Foot contact [V]: BF2 `FUN_001f3b90`, decomp lines 105563–105640, looks up the three animation CRCs
`f63b8713`, `c3ca09f4`, `758bc3c3` (`bone_l_leg2`, `bone_r_leg2`, `bone_b_leg2`) and reads matrix Y
at +0x34. Each strict downward crossing from above 0.48 to below 0.48 starts the class sound at
+0xe40/+0xe48/+0xe50. Setter hashes `21f56ed0`, `937ceaa5`, `12b23362` select those three slots
(decomp 112659, 112933, 112589); both native CIS class records name `cis_inf_droideka_step` there.
The port caches three model-space bone heights per baked clip frame at load, then interpolates
three floats at the simulation clip time. It suppresses humanoid footsteps for droidekas and
resets contact history for death, handle reuse, rolling and leaving draw/earshot eligibility.

Named presentation approximation: interpolating cached scalar heights is not evaluating the native
live blended skeleton; fractional-frame contact times can differ. The native alternate timer branch,
surface-extra sounds and leg hydraulic creak are not claimed here. Native foot contacts and class
sounds are checked against both games' baked rigs, without a new audio-device or live capture run.
