# OPUS RE (2026-10-22) - [O] UNAUDITED

# RE: the look stick's turn ramp (BF1 + BF2) — what drives it, what resets it, its two ends, who has it

Card LOOKR (loop lane 2, xhigh). Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c` (B2), both
ELFs (vtable words, small leaf functions read as instructions), the baked classes, 153 BF1 save states, and a live PS2
(PCSX2, software renderer, the RTX was lost): 12 scripted pad runs, one row per game update with the ramp field read from
RAM. Tags: [V] read (lines cited, each in the function named — check 1 below), [M] measured live, [D] derived, [H]
hypothesis; `[O]` = load-bearing. Scratch, tools, rows: `engine/artifacts/lead_tmp/lookr/`.

Conventions: BF1 soldier O, its controllable K = O + 0x2a0, class C1 = *(O+0x958); BF2 soldier E, its controllable
K2 = E + 0x270, class C2 = *(E+0x4a4). "Full" below means the game's own stick word after its clamp to [-1, 1].
Turn stick = the controller's axis 2 (K+0x68 / E+0x2e4), pitch stick = axis 3.

## 0. Headline [O]

**A human's yaw rate climbs from MaxPitchSpeed to MaxTurnSpeed over one second of a FULL look stick, in both games; any
stick short of full turns at the MaxPitchSpeed end, scaled by the stick.** Pitch never ramps. Bots never ramp.

| | BF1 | BF2 |
|---|---|---|
| the timer | K+0x120 (= O+0x3c0), in the player controller FUN_0025e098 | K2+0xf0 (= E+0x360), and a twin K2+0xf4, in FUN_002b0548 |
| it climbs (+dt per update, cap 1.0) while | the turn stick is at +-1.0 (or the left stick's x, when the class's MaxStrafeSpeed is 0) | turn^2 + pitch^2 >= 0.9 (the right stick's radius, any direction) |
| otherwise | 0 at once | 0 at once (both fields) |
| human yaw rate (rad/s) | S x (MaxPitchSpeed + (MaxTurnSpeed - MaxPitchSpeed) x timer) x stick | (MaxPitchSpeed + (MaxTurnSpeed - MaxPitchSpeed) x timer) x stick |
| S, the look sensitivity | 0.19 n + 0.4, n = the profile's option (1..10 [H]), **5 (x 1.35)** in every state | none |
| rifleman's two ends | 0.675 -> 5.4 rad/s (**38.7 -> 309.4 deg/s**; first update 47.7) | 1.0 -> 5.0 rad/s (**57.3 -> 286.5 deg/s**; first update 64.9) |
| pitch rate (rad/s) | S x MaxPitchSpeed x stick (38.7 deg/s full) | MaxPitchSpeed x stick (57.3 deg/s full) |
| a bot's yaw rate | MaxTurnSpeed, flat | **4 x MaxTurnSpeed**, flat |
| zoomed | every rate / the weapon's divisor (`re-zoom.md` 6); the timer runs the same | the same |
| the aim assist's own rate R | **MaxPitchSpeed** (not MaxTurnSpeed) | **MaxPitchSpeed** |

Class data: no soldier class of either disc authors MaxPitchSpeed, so it is the class constructor's default, **BF1 0.5,
BF2 1.0**; MaxTurnSpeed is authored (BF1 4.0, gun_inf_* 2.25; BF2 5.0, heroes 6.0 / 3.0) over the default 2.0.

## 1. The timer — the player controller [V][M][O]

**BF1 FUN_0025e098** (vtable-called, the ELF word 0x53851c; `re-aim-assist.md` 3.1). The four stick floats come from the
controller record (B1:179621-179624); the move pair is clamped to [-1, 1] every update (B1:179677-179692), the look pair
only in pad mode (a local == 0, B1:179693-179706; a local = FUN_00311250, the remote-input mode). Pad mode only:
- the timer K+0x120 = min(K+0x120 + dt, 1.0) (B1:179732-179734) when the turn stick (axis 2, a local) is <= -1.0 or
  >= 1.0, or when the controlled object's MaxStrafeSpeed is 0.0 (B1:179709-179710, section 6) and the left stick's x
  (axis 0) is at +-1.0;
- else K+0x120 = 0 (B1:179722).
- It runs before the aim assist (FUN_0025fea8, B1:179737) and before the turn stick is stored for the soldier (K+0x68,
  B1:179740): the test sees the clamped stick, not the assisted one.
- No soldier class has MaxStrafeSpeed 0 (census, section 6), so for a soldier the timer runs only on a full turn stick.

**BF2 FUN_002b0548** (vtable-called, the ELF word 0x621a14; `re-hover.md` 5.2 read the same lines for the hovers). The
four sticks (a local[0x2c], B2:234779-234783) clamped to [-1, 1] (B2:234784-234816; +-16 for the look pair when the mouse
flag FUN_003eef38 and a local - a soldier, `re-hover.md` 5.2 - are both set). Pad only (a local == 0, B2:234817): if
turn^2 + pitch^2 >= 0.9 (B2:234818; 0x3f666666 at 0x2b09f0 / 0x2b09f4) then K2+0xf0 and K2+0xf4 = min(+ dt, 1.0)
(B2:234819-234824), else both 0 (B2:234826-234828); then the aim shaping FUN_002b4598 (B2:234831). +0xf4 equals +0xf0
on every live row (216 / 216); its reader is not among the soldier's functions [H: what reads it].

**Live [M]** (`lead_tmp/lookr/ps2/out/<game>_full/rep0.csv`, `tools/rows.py`; the RAM timer and the game's own stick words
per update):
- A held full stick: the timer is dt on the first update (BF1 0.0334, BF2 1/30), +dt each update, 1.0 from update 30.
- A release of 2 updates resets it to 0; the next press starts at dt again (both games).
- An instant reversal (right released and left pressed on one update): the timer keeps climbing (both games; the stick
  word goes +1 -> -1 with no zero row). A real stick passing through the centre resets it.
- The pitch stick alone at full (20 updates): BF1's timer stays 0 and a turn added after starts at dt; BF2's climbs to
  0.667 and the turn starts at 0.70 (its test is the radius).
- **How full is full** (PCSX2 AxisScale s, the turn stick word the soldier received, the timer):

| s | 0.5 | 0.8 | 0.9 | 0.95 | 0.98 | 1.0 (key) |
|---|---|---|---|---|---|---|
| BF1 stick word | 0.4980 | 0.7961 | 0.8980 | 0.9451 | 0.9765 | 1.0 |
| BF1 timer | 0 | 0 | 0 | 0 | **0** | climbs |
| BF2 stick word | 0.2829 | 0.7087 | 0.8543 | 0.9216 | 0.9664 | 1.0 |
| BF2 timer | 0 | 0 | 0 | 0 | **climbs** (0.934 >= 0.9) | climbs |

  (the turn stick alone; on a diagonal BF2's radius test passes at s = 0.9 and 0.95 too - both words 0.85 / 0.92, and
  the look pair is then scaled into the unit circle, 0.7071 each, B2:234831-234863 per `re-hover.md` 5.2). BF1's stick
  word is near-linear in the throw and its timer needs the clamp itself (the last ~2 % of the throw); BF2's stick has a
  curve with a dead zone near 0.3 of the throw and its timer starts at about 97 % of it. Every partial run's yaw is the
  base rate x the stick word on every update.

## 2. The yaw rate [V][M][O]

**BF1 FUN_002098b8** (the soldier's yaw; 11 calls from the soldier update FUN_00205cc8, each passing the stick times the
stance's factor): with the player index O+0x388 >= 0 and not the remote-input mode (FUN_00311250, B1:119331-119339):
rate = C1+0x1844 + (C1+0x1840 - C1+0x1844) x O+0x3c0 (B1:119344-119345), x FUN_0034caf0(profile byte +0x15c) (B1:119361-
119368; FUN_0034caf0 = n x 0.19 + 0.39999998, B1:336894-336899) - offline: the flags tested at B1:119346-119360
(DAT_00518be4 / be8 / c08) are 0 in the ELF's data and in all 153 BF1 states, so the profile branch runs (a network game
can take FUN_002fc690's per-player value instead, B1:119356); a bot (index < 0) takes C1+0x1840 alone (B1:119340-119341)
and skips the zoom divide. A human's rate is divided by the weapon's +0xdc when zoomed (B1:119373-119376); yaw -= stick x
rate x dt (B1:119377-119378). The remote-input mode (FUN_00311250 != 0) takes the stick as the angle itself (rate 1.0,
no dt, B1:119335-119338, 119380-119381).
O+0x3c0 is K+0x120: K's vtable 0x532be0 carries the offset word 0xfffffd60 (-0x2a0) and its thunks subtract 0x2a0
(0x210c60, FUN_00210ca0 B1:123857).

**BF2 FUN_00247fa8** (11 calls from the soldier update FUN_00240bf8): with E+0x330 >= 0 and no mouse: rate = C2+0x758 +
(… / the weapon's +0xc8 when zoomed
(B2:162921-162926, a human with a weapon in hand only); returns -stick x rate (B2:162927). **No sensitivity factor.**
FUN_00238068 = C2+0x754 for a human, C2+0x754 x 4.0 for a bot (B2:151844-151851; 0x3c014080 at 0x23807c, the multiply
in the return's delay slot 0x23808c). A human under the mouse flag takes FUN_00238068 flat (B2:162912-162913).

**Live [M]:** every turning update of all 12 runs fits these laws within 2 % (BF1 140 + 5 x 60, BF2 the same: 880 of 880),
and the timer law fits every update (1380 of 1380). Control: the same rows against the port's rule (MaxTurnSpeed x 0.8
x stick, flat) fit 4 / 140 (BF1) and 8 / 140 (BF2) (`tools/rows_flat_control.py`).

## 3. Pitch [V][M]

BF1 FUN_00205cc8: rate = C1+0x1844 (B1:117163-117164), / the zoom divisor (B1:117170-117180), x the same S for a player
(on the stick x rate product, B1:117293-117306); clamp +-0.4 rad in state 2 (prone, B1:117328-117335), else [C1+0x1848,
C1+0x184c] (B1:117337-117344; = PitchLimits, -0.8 / +1.1 rad on every player class, `re-aim-assist.md` 10.3). BF2
FUN_00240bf8: C2+0x758 (B2:159984-159989). Neither reads the timer ([receipt:a3df543f] [receipt:47fc84f9]). [M] BF1
38.7 deg/s, BF2 57.3 deg/s at full, flat, x stick.
The card's note (a BF2 throw at 63.6 deg against the port's 63.0 clamp): the aim pitch itself stops at +1.1 rad = 63.03
deg; the 0.6 deg is outside the pitch clamp (where the throw leaves from) [H], not read here.

## 4. Bots [V][D]

The AIs write the same sticks through a converter that is handed the class rates: BF1 FUN_0020aad0 -> FUN_00233500
(C1+0x1834, +0x1838, +0x1840, +0x1844; B1:119906-119907), BF2 FUN_00249f80 -> FUN_002a54d8 (FUN_00238068(E) = 4 x
MaxTurnSpeed for a bot, and C2+0x758; B2:164694-164697). So a BF1 bot turns at MaxTurnSpeed flat (4.0 rad/s, the port's
rule today), **a BF2 bot at 4 x MaxTurnSpeed (20 rad/s for a 5.0 class; the port: 5.0)**, neither ramps, neither has S.

## 5. The aim assist's rate R [V][O]

BF1 FUN_0025fea8 halves both look sticks and returns when 1 - |turn| <= 0 (B1:181009-181016), else R = K's vtable slot
+0x30 (B1:181017) = 0x532c10 -> FUN_00210ca0 -> FUN_00201298 (B1:112998-113004) = **C1+0x1844, MaxPitchSpeed**. BF2
FUN_002b4598: R = slot +0x3c of K2's vtable (B2:236886); the soldier ctor FUN_00234a68 installs 0x61e998 at E+0x284
(B2:149409), 0x61e9d4 holds FUN_002558d8 -> FUN_002399f8 (B2:172400, 152897-152903) = **C2+0x758**. `re-aim-assist.md` 5
had R as MaxTurnSpeed [hypothesized]; the port's `aim_assist` takes `max_turn_speed`. [D] It is consistent: the assist
only acts below a full stick, where BF1's timer is 0 and the soldier turns at the MaxPitchSpeed end.

## 6. Class data [V]

| | B1 (setter FUN_0020f2e0, ctor FUN_0020d238) | B2 (setter FUN_00252bb0, ctor FUN_00250488) |
|---|---|---|
| MaxTurnSpeed 0x5433f589 | C1+0x1840 (B1:123113-123114), default 2.0 | C2+0x754 (B2:171785-171786), default 2.0 |
| MaxPitchSpeed 0xb5534be0 | C1+0x1844 (B1:123483-123484), **default 0.5** (B1:121804, 121822-121823) | C2+0x758 (B2:171158-171159), default 1.0 (B2:169396-169397) |
| MaxStrafeSpeed 0x3437a33d | C1+0x1838 (B1:123008-123009), default 7.0 | — |

The value tested at B1:179710 is C1+0x1838 (the soldier ctor FUN_001fee80 installs K's vtable 0x532be0 at O+0x534 and
O's 0x532d50, B1:111408, 111412): K's slot +0x18 = 0x210c60 (`jr ra; addiu v0, a0, -0x2a0`, returns O), O's vtable
0x532d50 slot +0x54 = 0x20d1d0 (`lw v0, 0x958(a0); jr ra; lwc1 f0, 0x1838(v0)`).
Census of the bake (`tools/census.py`, `checks/census.txt`; BF1 46 soldier classes, BF2 130):
no soldier class authors MaxPitchSpeed or PitchLimits [receipt:f2b46c1d] (the receipt's 64 / 177 bake hits of the two
keys are 9 / 55 and 35 / 88 non-soldier classes by the census program - the search finds the keys; second reader in 9);
every soldier's MaxStrafeSpeed is 4.0..6.6. The BF1 profile's sensitivity byte (EE 0x643718 + 0x15c, both profiles) is
5 in all 153 BF1 save states (the Auto Aim word beside it reads 1) [V-RAM]; the shell writes it through
`ScriptCB_SetControlScale` (B1:19650-19670), and the mod tools' copy of the shell script
(`community_mods/gametoast_lua/doc/StarWarsBattlefront/SourceCode/Common/Scripts/ifs_opt_controller_vehunit.lua`
827-851) steps it 1..10 - the PS2 disc's own script is not read, so the range on the PS2 is [H] (x 0.59 .. x 2.3).

Census numbers: BF1 MaxTurnSpeed 4.0 x 43, 2.25 x 3 (gun_inf_defender / rider / soldier); BF2 5.0 x 114, 6.0 x 11
(com_jedi_default and the heroes), 3.0 x 3 (Dooku, Vader, the Emperor), 4.0 x 2.

## 7. The port today (main 3ae13d3) [V-code]

- `core/src/soldier.rs` 642: yaw -= turn x max_turn_speed x stance factor x dt, from tick 1, for humans and bots alike;
  648: pitch at max_pitch_speed. `SoldierConstants::default` (109) has max_pitch_speed 1.0 for BOTH games: a BF1 soldier
  pitches at 2 x the PS2's (0.5), the player and the bots (`ai/gunner_foot.rs` 233-234 write the sticks against it).
- `input/src/lib.rs` 374-375: the right stick is scaled by LOOK_SCALE 0.80 BEFORE the sim sees it. The maintainer's five
  snapshots: the largest |rx| the sim received is 26213 = 0.80 x 32767. So no look-stick test of "full" can pass on a
  pure turn: **the vehicles' full-stick timer (`sim/seats.rs` 117, already built from re-hover 5.2) never runs for a
  human's turn** (0.64 < 0.9; only a diagonal reaches it), and a human hover / walker keeps its PitchRate start —
  noted on card VTRN as the first suspect for the maintainer's "vehicles turn too slowly" (INBOX 10-21 item 5).
- `sim/input.rs` ~411: the assist's R = max_turn_speed (section 5: the PS2's is MaxPitchSpeed).
- No sensitivity factor (BF1's x 1.35).

## 8. What a build needs (card LOOKR, phase build)

1. A per-soldier look timer (0..1): + dt while the RAW look stick is full, else 0, cap 1 — BF1 rule |turn| >= 1, BF2
   rule turn^2 + pitch^2 >= 0.9 — tested before LOOK_SCALE and before the aim assist. Humans only.
2. Human yaw rate = S x (MaxPitchSpeed + (MaxTurnSpeed - MaxPitchSpeed) x timer); S = 1.35 in BF1 (n 5), 1.0 in BF2;
   pitch = S x MaxPitchSpeed. LOOK_SCALE (the maintainer's 0.80) stays one multiplier on the result; one constant switches the
   ramp off (the timer pinned at 1 = today's flat top rate).
3. BF1 soldier defaults MaxPitchSpeed 0.5 (`SoldierConstants::from_props` needs the game) — moves BF1 bots' pitch.
4. The assist's R = MaxPitchSpeed (both games).
5. The modern pad's "full": the PS2's zone is the very edge of the throw (section 1); read the look stick the way the
   left stick already is (`STICK_FULL` 0.95 reads as full, the maintainer's pad peaks at 0.984), then apply the PS2 rule. A
   circular gate's diagonal never gives BF1's |x| = 1: the DS2's output at a diagonal is [H] (PCSX2's keys give (1, 1)).
6. The vehicles' `driver_input` takes the same raw full test (card VTRN measures before / after).
Not this card: BF2 bots' 4 x MaxTurnSpeed (section 4) — card BTURN.

## 9. Checks run, the verifier, what stays [H]

- **Check 1** (`re_check.py fn`): every B1 / B2 line cited above lies in the function named (66 B1 + 36 B2 lines,
  `lead_tmp/lookr/checks/fn_b{1,2}.txt`; the verifiers moved three citation ranges: the BF1 clamps, the prone clamp,
  the pitch S).
- **Check 2** (absence): the pitch functions read no timer - `0x3c0)` / `[0xf0]` in B1 hit FUN_002098b8 only among the
  soldier's functions [receipt:a3df543f]; `0x360)` / `[0xd8]` in B2 hit FUN_00247fa8 only [receipt:47fc84f9]; both
  readers also ruled out a read through K (+0x120 / [0x48], +0xf0 / [0x3c]). The timer's writers inside the six
  functions are only the four cited (both readers, scripted range scans). BF2's +0xf4 has no reader among them
  [receipt:00b2dd30]. No soldier class authors MaxPitchSpeed: `tools/census.py` (a program-side count) plus both readers'
  own parses and their attribution of all 13 / 115 raw-disc PROP chunks of 0xb5534be0 (propscan) - every one falls in
  a building, turret or walker class.
- **Check 3** (the twins): B1 FUN_0025e098 / B2 FUN_002b0548 - no direct caller in either corpus, one ELF data word each
  (0x53851c / 0x621a14); B1 FUN_002098b8 / B2 FUN_00247fa8 - 11 callers each, all in the soldier update (B1 FUN_00205cc8,
  B2 FUN_00240bf8); the AI converters B1 FUN_00233500 (6 callers) / B2 FUN_002a54d8 (7).
- **Check 4** (ELF words): 0x532be0 = 0xfffffd60, 0x532bf8 = 0x210c60, 0x532c10 = 0x210ca0, 0x532d50 + 0x54 =
  0x20d1d0, 0x210c60 = 03e00008 2482fd60, 0x20d1d0 = 8c820958 03e00008 c4401838; B2 0x61e998 = 0xfffffd90, 0x61e9d4 =
  0x2558d8; 0.9 = 0x3c013f66 / 0x34216666 at 0x2b09f0; 4.0 = 0x3c014080 at 0x23807c; 0.19 / 0.39999998 at 0x34caf4 /
  0x34cb04; the ctor stores of +0x1838 / +0x1840 / +0x1844 at 0x20d548-0x20d550 and of +0x754 / +0x758 at
  0x2507e0-0x2507e4. One verifier traced the registers at 0x25e634 and 0x25ffd8: the vtable calls pass this = K.
- **Check 5** (writers): the BF1 flags DAT_00518be4 (4 writes, all 0 or a copy, FUN_00174cf8 / FUN_002fd018 /
  FUN_002fdf70), DAT_00518be8 (set to 1 only in FUN_00322f20 / FUN_00322f48, network setup), DAT_00518c08; data words 0;
  0 in every state.
- **Verifier** (the workflow `lookr-re-verify`, 5 agents at medium effort, claims and citations only): batches of 9 (BF1
  code), 7 (BF2 code), 8 (live rows and data); the 2 twin and 3 absence claims to two separate readers. **34 verdicts:
  26 SUPPORTED, 8 PARTLY, 0 NOT SUPPORTED.** The PARTLYs were wording or citation and are fixed above: the clamp
  ranges (A1, A7), "only s = 0.98 ramps" holds for the turn stick alone (M5), the flat control carried BF1's S (M6: 2
  -> 4 of 140, re-run), the BF2 census split (M8: 5.0 x 114, 4.0 x 2), the twins' "both clamp all four sticks" (T1, both
  readers: the look pair only in pad mode / +-16 under the mouse) and "both divide by the divisor" (T2, one reader: a
  human only). The second reader overturned no claim; on T2 it split SUPPORTED / PARTLY and the PARTLY's point is taken.
  Verdicts: `lead_tmp/lookr/verify/verdicts.json`.
- **Stays [H]**: what reads BF2's +0xf4; the PS2 disc's own sensitivity range (1..10 from the mod tools' script); how a
  real DualShock 2 reads at a diagonal (PCSX2's keys give both axes 1.0); the 0.6 deg of the GRNP throw above the
  pitch clamp; BF2's stick curve between the measured points.
