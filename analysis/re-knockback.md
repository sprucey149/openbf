# OPUS RE (2026-10-07) - [O] UNAUDITED
# RE: blast knockback and the death kick - who a blast pushes, what a pushed soldier does, the droid chunk death (card KNOCK)

> **AUDIT 2026-10-21 (loop lane 1, PARTLY: `engine/artifacts/loop/audits.md`; follow-up card KNOCK2).** Two separate
> verifiers checked 17 claims against the decomp, and none was refuted. The PS2 rows of §6 were recomputed with a new
> parser, and the ELF words were decoded.
>
> Held: §1-§3 for BF1 (the push and its skip set, the apply, the state setter's refusal, the splash kick, the direct
> hit's kick, the thrown / tumble / get-up timings and thresholds, the brake, the lock, the corpse, the death block);
> §4's PushDeadOnly gate and its default; §2's clip ids.
>
> Corrections:
> 1. §4 "States": BF2's tumble (10) and get-up (0xb) do not brake like BF1's. FUN_00240bf8 (B2:160735-160812) passes
>    the air integrator FUN_00248560 an acceleration of -2.5 (tumble) or -10 (get-up) times the velocity along the
>    body rows, plus the stick. There is no floor test. BF2's corpse brake (FUN_002480a0) also works in the air, and
>    on the floor it adds the slope's gravity shift (one verifier; the air part from both).
> 2. §2 / §4: the speed^2 tests at the thrown landing, the tumble's end and the get-up's end are horizontal
>    (x^2 + z^2). The B1 sites 0x207aa8 / 0x207e40 / 0x207f14 are `vmul.xz` then `vaddz.x`. The B2 sites per one
>    verifier: 0x243488 / 0x244278 / 0x2448e8 / 0x244a8c. The tests at the moment of the knock keep the form given
>    in §1 / §4.
> 3. §7, counted as the port reads the classes (BASE inheritance): Push > 0 on 108 of 183 BF2 explosion classes, not
>    84, because 24 faction classes inherit the com_ class's push. "Every infantry weapon authors Inner = Outer" holds
>    for BF1 only: BF2's detpack is 2 - 3.5, timebomb 4 - 6, bulldog 1 - 2, incinerator 0 - 0.3 and arccaster
>    0 - 0.5. com_weap_inf_force_push_exp (PushDeadOnly 1, Push 0) is in the bake.
> 4. §2 clips: FUN_0022a0a8 picks a clip only when the state changes, and it reads the per-update displacement of the
>    body matrix (argument 1[5]), not S+0xa20. After ids 0x10 / 0x11 (the get-up clip of the lock) a death rests too.
> 5. §1: FUN_00209758 runs the zoom drop (S+0xc4c = 1) before its state-3 / same-state refusal. The lock clears the
>    trigger bytes whenever S+0xc0c > 0, not only in (0, 10).
> 6. Neither verifier found a state or lock gate on BF1's enter path (FUN_0020a4f0, FUN_0020b2c8): a thrown soldier
>    can board [H: the controller's vt+0x4c was not opened].
>
> Evidence: `engine/artifacts/lead_tmp/audit-knock/wf/results.json` (V1, V2, N1, N4).

**The question** (card KNOCK; `re-vehicle-death.md` §5 and its audit banner left it open): a blast with Push > 0
calls a soldier's push. What does the soldier then do - which state, which clip, how long, can he act; which five
states are skipped; when does a droid soldier blow apart instead and what spawns; what does the droid class do; and
is BF2's "only the dead" real in play?
**Method.** Read both games' soldier push, push apply, splash, death kick, direct hit, death callback, the death
block and the thrown / tumble / get-up / dead cases of the soldier update, the movement integrator and the animation
map; disassembled every float argument, vector mask and compare the build needs; then measured it on the PS2: a
scripted probe throws the player's grenade, puts held bots on a ring round it and logs every subject per game update
(`lead_tmp/knock/knock_probe.py`, 7 runs, 23 blasts, both games; §6). Verified by a 12-agent workflow (§9).
**Conventions.** B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c` (line numbers). [V] read
in the decomp, [V-instr] in the disassembly, [V-RAM] in a save state, [M] measured live on the PS2 (rows in
`lead_tmp/knock/out/`), [D] derived, [H] hypothesis. [O] = load-bearing and verifier-checked.
BF1 soldier S (vtable 0x532d50): health S+8, alive bit = bit 3 of S+0xac, velocity S+0xa20, state S+0xbe4, previous
S+0xbe8, time in state S+0xbec, posture S+0xbe0, flag byte S+0xbfd (bit 1 on the ground, bit 3 the chunk mark), the
action lock S+0xc0c, class S+0x958 (= S+0x290). BF2 soldier E (vtable 0x61eb88): health E+0x170, alive bit = bit 3 of
E+0x228, velocity E+0x550, state E+0x7e4, time E+0x7ec, flag byte E+0xa74, ground normal E+0x4b0, class E+0x4a4.

## 0. The answers

| # | question | answer |
|---|---|---|
| 1 | who is pushed | **BF1:** every gathered soldier whose state is not thrown (5), tumble (8), get-up (9) or mounted (12, 13) - the living, those this blast just killed and corpses (state 3) alike - and the `droid` class; nothing else. **BF2:** the same soldier function, but a blast whose class has `PushDeadOnly` (on by default) calls it only for an object whose alive bit is clear: a thermal detonator pushes no living soldier [M: 0 of 22], and with that one bit cleared in RAM it throws them all [M: 15 of 15]. **Two BF2 disc classes author PushDeadOnly 0 and push the living: the Ewok's rock and the wampa's smash** (§7; `re-vehicle-death.md` said none). |
| 2 | the impulse | `dv = (normalize(sphere centre - blast) + (0, 0.6, 0)) x Push x f`, f = `re-vehicle-death.md` §3's falloff on PushRadiusInner / Outer with the victim's sphere. Every infantry weapon authors Inner = Outer, so f is 1 or 0: all or nothing inside `PushRadius + w/2`. Measured to 0.00 m/s on subjects at rest (§6). |
| 3 | a living soldier (BF1) | velocity += dv; if its y > 2 he is **thrown** (5): ballistic at g = **13.5** (not 18), no drag, the stick at 20 %, no fire; touching the ground after >= 0.3 s he **tumbles** (8) when faster than 6 m/s (or than 3 m/s, 7 times in 10), else **gets up** (9); the tumble brakes at 0.12 x Acceleration (8.4 m/s2) until under 4 m/s (0.5 s at least, 2.5 s at most), the get-up at 0.24 x Acceleration until under 0.6 m/s (1 s at most); then he stands with a **1.9333 s lock** (no move, turn, fire or jump). y <= 2 but over 2 m/s along the ground: straight to the tumble. A thermal detonator 2 m away: 14 m through the air in 1.5 s, at rest 18.7 m away after 2.4 s, free after 4.4 s [M]. |
| 4 | the death kick | a soldier who is dead after a blast's damage step gets **+5 m/s along normalize(feet - blast)** - also a corpse killed earlier, at every blast that gathers it; a soldier a **direct hit** kills gets +5 m/s along the shot (23 of the 40 deaths in the audit's 60 s battle sample show 5.0 +- 0.15 m/s). A mounted one is dismounted and thrown at (d.x, d.y + U x 0.3 + 0.4, d.z) x 9. Same in BF2. |
| 5 | the dead are thrown too | a soldier the blast kills is pushed and kicked while his state is still 0, so he flies (5), tumbles (8), "gets up" (9), stands for one or two updates and only then dies (3): **BF1's death waits for the landing**, 2.5 - 3.6 s after the blast [M: 7 of 7; the 8 pushed dead droids 1.8 - 3.5 s]. BF2: thrown (9), tumble (10), get-up (11), then dead (0xf) with no standing update, 1.5 - 1.8 s [M: 4 of 4]. |
| 6 | corpses | BF1 state 3 / BF2 0xf last **4.0 s**, stay in the collision world, integrate their velocity (gravity 18; brake BF1 min(t,1)^2 x 0.6 x Acceleration, BF2 min(t,1)^2 x 100) and are damaged, kicked and pushed by every later blast [M: BF1 5 of 5 to 0.00 m/s; BF2 5 of 5, the 4 at rest to <= 0.05]. |
| 7 | droid soldiers (BF1 CW) | a class with chunk geometry rolls `ChunkFrequency` (0.6) at **every push call, before the distance test**: success = no push and a sticky mark (bit 3 of S+0xbfd) - even outside the radius; failure = pushed, and a dead one's health is set to -4. At the death the roll is taken again; success or the mark = **chunk death**: the class's `ExplosionName` blast at the body, the class's chunks launched at 0.1 + 0.018 x overkill (1.0 from 50 on), collision gone at once, no corpse. Live: 17 of 34 not pushed, each with the mark; 8 of 8 pushed dead at health -4.0 [M]. |
| 8 | the droid class | `EntityDroid` (BF1 vtable 0x52fa50): velocity += normalize(d) x Push x f, no upward term, no state or alive test (both games). |

## 1. BF1: the push [V][O]

The blast (`re-vehicle-death.md` §2) calls, per gathered object with a clear line: the splash `vt+0x80`, then the push
`vt+0x84` ($f12 PushInner, $f13 PushOuter, $f14 Push). EntitySoldier's slots (ELF words 0x532dd0 / d4 / d8):
FUN_00200e68, **FUN_002009a0**, **FUN_002008b0** (`vt+0x88`, the apply).
**FUN_002009a0** (B1:112578-112665; [V-instr] 0x2009a0-0x200c2c):
1. state S+0xbe4 in {12, 13, 5, 8, 9} -> return (B1:112599). Nothing else gates the impulse: the alive bit is never
   read, the state is not compared with 3, and the health is read only for step 2's -4.0.
2. if S+0x290 (the class) and class+0x320 (chunk slot 0's geometry) are set: one LCG draw U (DAT_0066eed0); `U <=
   class+0x1e58` (**ChunkFrequency**, hash 0x43b50014, setter B1:123077-123078) -> S+0xbfd |= 8, return - before any
   distance is computed. Else, if the health < -4.0: FUN_001bbb08(**-4.0**, S+8, 1, 0) ([V-instr] 0x200ad4-0x200af8:
   the decomp drops the float argument) stores -4.0 as the health and fires the health-changed callback with old -
   new (off-line; in a network session's pump a value <= 0 becomes 0.0001 first, B1:64045-64087).
3. d = sphere centre - blast (x, y, z); if |d| > 1e-4 and f = FUN_00221530(|d|, Inner, Outer, S) > 0:
   `vt+0x88(Push x f, S, normalize(d) + (0, 0.6, 0))` (0x3f19999a into the y word; not renormalised).
**FUN_002008b0** (B1:112535-112572; [V-instr] 0x2008b0-0x200998): S+0xa20 += dir x scale (xyz). Then, on the new
velocity: `v.y > 2.0` -> FUN_00209738 = … else `v.x^2 + v.z^2 > DAT_00517874^2` (`VMUL.xz`; ELF 2.0,
no writer) -> FUN_00209758(S, 8). On the thrown branch a human's soldier in a one-player game also gets S+0xbfe |= 0x10
and FUN_00448110(class+0x1de8, class+0x1df0, -1.0, ..) (B1:112556-112561; a sound or camera cue, not read [H]).
**FUN_00209758** (the state setter, B1:118842-118921) refuses when the current state is the new one or is **3**: a
corpse keeps state 3 and keeps the velocity it was given. (On a change it also clears S+0xc10 unless the old state
is 12 and stops a sound at S+0xc30; a request for 5 on a human's zoomed soldier first drops the zoom, S+0xc4c = 1,
even when the change is then refused.) State 5 has no other source: FUN_00209738 is called from 0x200948 (this
apply) and 0x20115c (the mounted death kick) only.
The five skipped states: **5 thrown, 8 tumble, 9 get-up** (§2) and **12 / 13 mounted** - written by the soldier's
seat update FUN_0020c880 (B1:121331-121335: 13 when either of two words of the mount's objects is set, else 12;
the lead's reading, not sent to the verifiers; which seats is not read [H]) and FUN_0020aad0 (12, B1:119808).
[M] eight living subjects forced into 5, 8 and 9 inside the radius took the blast's 320 damage and no push (§6).

## 2. BF1: thrown, tumble, get-up and the lock [V][O][M]

All in the soldier update FUN_00205cc8; the integrator is FUN_002098b8(dt, forward, strafe, turn, brake, S).
- **Thrown, state 5** (`case 4: case 5: case 6:` B1:117557-117622): trigger bytes zeroed (no fire); the integrator
  gets (stick x 0.2, stick x 0.2, turn x 0.5, brake 0) for the living and all zeros for the dead. Gravity in the air is
  **-13.5** in states 5 and 9 and -18.0 in every other state (B1:119278-119299). Leaves only when S+0xbec > 0.3 and
  on the ground: speed^2 > 36 -> 8; > 9 and an LCG draw > 0.3 -> 8; else 9 (B1:117586-117607); the BodyFall foley
  plays there (`re-foley.md`). [M] the player: vy 10.01 in the blast's update (position untouched); the next update
  moves him with no gravity (the ground flag is still set), then -13.5 x dt in each of 44 updates; he touches down in
  update 46 (1.535 s) and is in state 8 in update 47; horizontal 9.16 m/s unchanged throughout, apex +3.88 m.
- **Tumble, state 8** (B1:117692-117714): posture 1 (the crouch profile), no fire, integrator (0, 0, turn x 0.1, brake
  0.2); to 9 when S+0xbec > 0.5 and (speed^2 < 16 or S+0xbec >= 2.5). [M] 9.16 -> 3.83 m/s at 8.4 m/s2 in 0.634 s.
- **Get-up, state 9** (B1:117715-117737): posture 1, no fire, brake 0.4; when on the ground and (speed^2 < 0.36 or
  S+0xbec > 1.0): FUN_00209018 -> S+0xc0c = the length of special clip 0x10 `stand_getupfront` (58 frames) and state 0
  (B1:118536-118538, 118642). [M] 16.8 m/s2 for 0.234 s; the lock reads 1.9333 and runs out 1.935 s later.
- **The brake** (B1:119197-119214): with brake > 0, on the ground and the floor normal's y > DAT_005178ac (0.75), the
  forward and side speeds each fall by `Acceleration (class+0x1830) x dt x DAT_005178a4 (0.6) x brake`. Acceleration
  is 70 on the classes measured: 8.4 (tumble), 16.8 (get-up), 42 (standing, and a corpse after 1 s).
- **The lock** S+0xc0c (B1:117030-117100): while in (0, 10) the move, strafe, turn and pitch inputs are zeroed and the
  trigger bytes cleared; it falls by dt. So after the get-up the soldier stands locked for 1.93 s.
- **Clips** (FUN_0022a0a8 B1:142050-142232; name table ELF 0x517af0): thrown = `thrown_flyingfront / back / left /
  right` (ids 6-9) by the velocity against the body axes (a random value in +-10 is stored with it: a spin [H]);
  tumble = `thrown_tumblefront / back` (0xa / 0xb); get-up state = `thrown_bounce..soft` / `thrown_land..soft`
  (0xc-0xf); dead after one of those = the rest poses `thrown_restfrontsoft / backsoft` (0x12 / 0x13; also from id
  0x20 with its next word 2) instead of a `stand_death_forward / backward / left / right` clip (picked by two dot
  products of the hit direction). The lock's 1.93 s is where `stand_getupfront / back` plays [D].

## 3. BF1: the death kick, the late death, corpses, the chunk death [V][O][M]

- **Splash** FUN_00200e68 (B1:112791-112843): a rider whose mount is not on the building layer returns undamaged;
  the common splash; then **if the alive bit is clear** - whether or not this blast did it -
  FUN_00200f90(S, normalize(S+0x230 - blast)).
- **Death kick** FUN_00200f90 (B1:112849-112918): on foot S+0xa20 += dir x DAT_00517878 (ELF **5.0**, no writer).
  State 12 / 13: dismount (controllable vt+0x54), and unless the mount's vt+0x70 returns non-zero [H: which mounts]
  the velocity is SET to (d.x, d.y + U x 0.3 + 0.4, d.z) x 9.0 and the state to 5.
- **Direct hit** FUN_00200c30 (health interface vtable 0x532d20 slot +0x1c, B1:112695-112771): an already dead
  soldier takes no hit; a hit that kills calls FUN_00200f90(S, the hit's direction). [M] 23 of the 40 deaths in the
  audit's battle sample (`lead_tmp/audit-vdth/corpse_samples.csv`) show |dv| = 5.0 +- 0.15 m/s at the death.
- **The death block** (B1:116889-116968) is skipped in states 3, 5, 8 and 9. Otherwise, with the alive bit clear: one
  LCG draw U; `U <= ChunkFrequency` or the mark (S+0xbfd bit 3), on a class with chunk geometry, is the **chunk
  death**: FUN_002701a0(class+0x1e6c = `ExplosionName` (hash 0x9b30e763), sphere centre, {S+0x380, team, class, 0},
  0); the class's chunks FUN_001e2630(s, S) (the vehicle chunk launch, `re-vehicle-death.md` §6.1) with s = 1.0 at
  health <= -50, else -health x 0.018 + 0.1; FUN_00168810 (out of the collision world) and a call through
  (S+0x1fc)+0x10 on S+0x1d0 (hides the body [D]: FUN_0020c880 calls the same slot beside FUN_00168810 when it takes
  a soldier out of the world, and +0xc to bring it back); a network session's pump skips the chunks. Both ways end
  in the health interface's vt+0xc (thunk 0x211200) = **FUN_001ffec0**: the item drop (class+0x1e10 entries); a
  rider (collision layer 0x10) gets FUN_00200f90(S, 0x66eee0) - the engine's zero vector (its only direct stores are
  the static initialiser's 0.0 at 0x445fd4 - 0x446008; 72 sites pass its address on) - so he is thrown straight up
  at 3.6 - 6.3 m/s [D]; then state 3 unless the state is 5.
- So **a soldier killed by a blast dies late** [D][M]: the blast clears his alive bit, kicks and pushes him in the
  same update (state 0 -> 5); the death block then waits out 5, 8 and 9; one or two updates after the stand it runs
  and sets state 3. 7 of 7: dead 2.50 - 3.57 s after the blast. The item drop and the kill's `thrown_rest` pose come
  then; the damage and the kill credit are at the blast.
- **Corpse, state 3** (B1:117521-117555): posture 2; while it has a collision body the integrator runs with brake
  min(t, 1)^2 (gravity -18 in the air); at t > 4.0 the body leaves the collision world and (off-line) the update
  returns 0: the soldier is released. `re-vehicle-death.md`'s fade (3 - 4 s) is the draw's. [M] a corpse 0.9 s old,
  2.35 m from the blast: dv = push + kick = (-14.91, 7.22, 0.00) to 0.00 m/s, health -1 -> -321 (it takes the damage
  again), apex +1.57 m, at rest 12.8 m away after 1.13 s (42.7 m/s2 on the ground).
- **The mark is sticky**: set only at B1:112609, never cleared alone; the byte is zeroed at the soldier's reset
  (FUN_001ff1e8, FUN_0032f820). A droid that was inside any push blast's gather box and rolled <= 0.6 blows apart
  when it dies, however much later [D][M: the bit stays set across blasts and is clear after the respawn].
- [V-RAM] S+0x290 = S+0x958 on every soldier; class+0x320 is set on exactly the four CIS droid classes, each with
  ChunkFrequency 0.6 and ExplosionName `com_inf_droid_exp` (disc: an Effect `droid_explosion` only - no damage, push
  or chunks of its own). Other classes hold garbage at class+0x1e58 in RAM; both readers gate on the geometry first.

## 4. BF2: the twins and what differs [V][O][M]

([O] except the death kick's and the death block's bullets: [V], one of two verifiers PARTLY - §9.)

Same slots one step on (vtable 0x61eb88: +0x8c splash FUN_00239480, +0x90 push FUN_00238b80, +0x94 apply
FUN_002388b8), same LCG pattern (DAT_0073d870).
- **The gate is in the blast** FUN_0030c8f8 (B2:300680-300699): splash first; then with class+0xa0 bit 0
  (**PushDeadOnly**, hash 0x26d8bec1, constructor default on) an object whose alive bit is set gets no push call. BF1's
  ELF and disc have no such hash. BF2's disc: §7 - three explosion classes author it, two of them as 0.
  [M] the causal test: the thermal detonator's flag byte 0x0f -> 0x0e in RAM (`--dead-only-off`) and all 15 living
  subjects inside the radius were pushed and thrown, the player at rest to 0.00 m/s.
- **FUN_00238b80** = FUN_002009a0: skipped states 9, 10, 11, 0x11, 0x12 (thrown, tumble, get-up, mounted); the chunk
  roll on class+0x68 / class+0x121c with the mark at E+0xa74 bit 3 and the -4.0 health; the same impulse.
- **FUN_002388b8, the apply, is not BF1's** (B2:152167-152287; [V-instr] 0x2388b8-0x238b78): nothing if E+0x4ac is set
  [H: what]; if v.dir < 0 half of that opposing part is removed first (`v -= 0.5 (v.dir) dir`, dir not normalised;
  [M] for the 9 subjects walking against the push the error falls from a median 3.52 to 0.77 m/s with it);
  v += dir x scale. Then by state: thrown -> nothing more; jet hover (7) and jump / jet / fall (4, 6, 8) take a
  flinch (E+0x4e4, flag bits 0x3c00; what it shows is not read [H]) below scale 8 and 5, else are thrown; on the
  ground `v . floor normal > 3.0` -> **thrown (9)**, else `|v|^2 > 64` (3-D) -> **tumble (10)**, else scale > 2 -> a
  0.5 flinch (a sprinter is stopped). From a PushDeadOnly blast only the dead reach it.
- **Death kick** FUN_002395a8: +5.0 (a literal) along the direction, and a human's soldier below health -100 also
  gets … mounted (0x11 / 0x12): the x 9 throw. Called from the
  splash, the direct hit FUN_00238e20 (B2:152580-152613), the death callback FUN_002379f0 (rider layer 0x20, the
  zero vector) and three sites in FUN_00315f78 (zero vector; not read).
- **States** (FUN_00240bf8): thrown 9 lands like BF1's (36 / 9 / 0.3, B2:160614-160653) but flies with the air
  integrator FUN_00248560 (gravity 18 and its drag, `re-soldier-ground.md`); tumble 10 ends at 0.5 s under 4 m/s or
  at **0.75 s** (BF1 2.5); get-up 0xb ends under 0.6 m/s or after 1.0 s. The death block skips 0xf, 0x10, 9, 10, 11
  and has the same chunk branch (class+0x1238 ExplosionName, FUN_00213630, the same scale, the body out of the
  world and hidden; the death call at B2:159669) (B2:159605-159669).
- **Dead**: 0xf integrates with brake min(t,1)^2 x 100 while it has a body; 0x10 (classes with class+0x330 bit 33 or
  class+0x778 bit 4 [H: heroes]) does not move; both leave the world at 4.0 s (B2:160981-161039).
- [M] BF2 living, pushed (the bit cleared): thrown 0.6 - 1.5 s, tumble 0.53 s, get-up 0.03 - 0.2 s, then state 0
  (15 of 15); BF2's stand-up lock after it is `re-anim-banks.md`'s (1.9 s), not re-read here.
- [M] BF2 Hoth, thermal detonator (Damage 450, radii 4.5 - 6.5, Push 10, PushRadius 6.5 - 6.5, flags 0x0f): 22 living
  subjects of both teams inside the radius (the player among them) took the damage and **0.00 m/s**; 4 killed by the
  blast and 5 corpses (0.3 - 2.9 s old) got push + kick (median error 0.01 - 0.56 m/s; the misses had a walking speed
  of their own). A corpse older than 4 s took neither damage nor push (gone from the world).
- Not traced: the other callers of BF2's apply (seven virtual call sites through slot +0x94: 0x1f4f84, 0x241288,
  0x25e9f0, 0x31838c, 0x32d3b8, 0x32d3ec, 0x341b94) - what else knocks a living BF2 soldier down (Force powers,
  vehicles) is there [H].

## 5. The droid class (both games) [V]

BF1 vtable 0x52fa50 = `EntityDroid` (RTTI), +0x84 = FUN_001c8590 (B1:72968-73034): with d = centre - blast,
entity+0x580 += normalize(d) x Push x f. BF2 FUN_001ed7c8 (B2:100780-100846; entity+0x440; the class registered as
"EntityDroid") is the same op for op. Every other damageable class's slot is `jr ra` (`re-vehicle-death.md` §9):
vehicles, buildings, the droideka (BF1 `walkerdroid`) are never pushed.

## 6. Live on the PS2 [M]

`lead_tmp/knock/knock_probe.py bf1|bf2 --state <a *_spawn.p2s> --plan ...` (SW renderer; ~1 min a run): the player's
soldier gets health 30000 and looks down; L1 throws his grenade; when the OrdnanceSticky ARMS (its first contact),
the nearest soldiers are put on a ring round it and held in x / z until 0.1 s before the blast: `L` living (health
30000), `B` health 1.0 (the blast kills), `C` alive bit cleared at arming (a corpse), `X` outside the radius, `S / T / G`
forced into tumble / thrown / get-up; `--double` a second grenade on the same subjects; `--knockin 10,4.5,4.5`
writes a Push into the grenade's explosion class (the Republic's grenades have none) and `--enemy --droids` takes
droid subjects; `--dead-only-off` clears BF2's PushDeadOnly bit in the grenade's explosion class. One row per game
update and subject. `knock_ana.py` / `droid_ana.py` / `live_summary.py` print the tables (`out/live_summary.txt`).
Runs (`out/*.json`): BF1 Hoth x 3, BF1 Tatooine CW (droids), BF2 Hoth x 3.

| game | subject inside the push reach | n | pushed | error of dv against the model, m/s |
|---|---|---|---|---|
| BF1 | living | 17 | **17** | median 0.09; the 6 at rest before the blast: 0.00 |
| BF1 | living, forced into thrown / tumble / get-up | 8 | **0** | 0.00 (damage taken: 320) |
| BF1 | killed by the blast | 7 | **7** | push + kick: median 0.76 (all walking) |
| BF1 | corpse before the blast (0.5 - 0.9 s old) | 5 | **5** | push + kick: 0.00 |
| BF1 | outside the radius (control) | 6 | 0 | 0.00 |
| BF1 droid | living | 19 | 9 | pushed 0.03; not pushed: mark set 10 of 10 |
| BF1 droid | killed by the blast | 15 | 8 | pushed: health -4.0, 8 of 8; not pushed: kick only + mark, 7 of 7, state 3 next update |
| BF2 | living (both teams, the player) | 22 | **0** | 0.00 (damage taken: 450 or its falloff) |
| BF2 | killed by the blast | 4 | **4** | push + kick: median 0.56 |
| BF2 | corpse before the blast (0.3 - 2.9 s old) | 5 | **5** | push + kick: median 0.01 |
| BF2, PushDeadOnly bit cleared | living | 15 | **15** | median 0.71 with the half-removal rule; the 2 at rest 0.00; all thrown |

The model: `dv = (n + (0, 0.6, 0)) x Push x f` with n from the blast to the sphere centre (feet + 1.0 standing, + 0.35
for a corpse; w 1.4626 / 1.6221, `re-vehicle-death.md` §3), plus `5 x normalize(feet - blast)` when dead after the
splash. A droid outside the push radius but inside the gather box was marked too (2 of 4): the roll precedes the
distance. Reference rows for the port's tests: `out/bf1_hot1_knock.json` trial 2 (P, C1, B1), `bf1_hot1b` trial 1
(two blasts), `bf1_hot1c` (forced states, the lock field), `bf2_hot1b` trial 2.
Limits: one map and one grenade class per game; thrown-state air control and the 70 % tumble draw are read, not
measured; no picture was compared (the clips of §2 are the map function's ids, not frames).

## 7. The disc [V]

**PushDeadOnly** (a raw scan of every file of both extracted discs for the hash's bytes; the verifiers', reproduced):
BF1 0 hits in 289 files. BF2 17 hits: `com_weap_inf_force_push_exp` = 1 (Push 0); **`ewk_weap_inf_rock_exp` = 0**
(SIDE/EWK.LVL: Damage 25, radii 0.2 - 0.3, Push 4, PushRadius 0.2 - 0.3) and **`hoth_weap_inf_smash_exp` = 0**
(SIDE/SNW.LVL: Damage 800, radii 4 - 6, Push 8, PushRadius 3.5 - 4.5, HurtOwner 0); and an ordnance class
(`com_weap_inf_mortar_launcher_ord1` = 0), where the key is dead: only the explosion setter reads the hash.
`re-vehicle-death.md` §5 / §7's "no disc class authors 0" came from `expc.py`, which reads the baked class data - and
the bake holds neither the EWK nor the SNW side.
Push > 0 on 89 of 119 baked BF1 explosion classes and 84 of 183 BF2 ones (`lead_tmp/knock/out/push_table.txt`, from
`lead_tmp/re/vdth/expc.py`; the unbaked sides are not in that count). BF1 infantry: thermal detonator, land mine
10 / 4.5; mortar 10 / 3.5; wrist rocket, remote charge, seismic mine 10 / 2.5; rocket launcher 10 / 0.75; orbital
strike 10 / 10.5 - Inner = Outer on each; the haywire and crack grenades have none. BF2: thermal detonator
10 / 6.5, rocket launcher 10 / 4, time bomb 16 / 4 - 6, detpack 12 / 2 - 3.5. Vehicle `_exp`:
`re-vehicle-death.md` §7 (13 / 7 ... 13 / 50).

## 8. For the port (card KNOCK, build)

- **core, the blast** (`sim/blast.rs`, the marked spot): after a victim's damage, the death kick when it is dead; then
  the push when `push_outer > 0` - BF1 maps: every soldier not thrown / tumbling / getting up / mounted, alive or not;
  BF2 maps: the class's `PushDeadOnly` (read the prop; default on) decides - only the dead for every weapon the port
  bakes today, the living too for the Ewok's rock and the wampa's smash, with §4's apply (half of the opposing
  velocity off, thrown at v.n > 3, tumble at |v| > 8, else a flinch). The same sphere and falloff as the damage.
  Bolt and rocket kills: the kick along the shot (`sim/combat.rs`).
- **core, the soldier** (`soldier.rs`): stances Thrown / Tumble / GetUp with §2's numbers per game (gravity 13.5 BF1;
  BF2's air integrator), the 0.3 s landing rule with its draw from the sim's LCG, the brakes, the get-up lock
  (1.9333 s BF1; BF2's stand-up lock is `re-anim-banks.md`'s 1.9 s, not re-read), no fire and no control beyond the
  stick's 20 %.
- **core, the dead**: a unit killed while pushed keeps flying and dies at the landing (BF1: after the stand; BF2: from
  the get-up); a dead unit keeps a body for 4.0 s that integrates (§3 / §4 brakes), blocks nothing new, and is in the
  blast gather for damage-less kick and push. Today a dead unit is frozen and skipped by the tick.
- **Droids** (new card DCHK): the ChunkFrequency roll in the push, the mark, the chunk death (explosion effect +
  soldier chunks). Until then a BF1 droid is pushed like a clone - a named, temporary deviation.
- **game**: the thrown / tumble / rest clips by §2's map (the animator has only `stand_death_forward` today); the
  corpse's draw follows the sim's body.
- **declares:** `sim; sweep; arena` (blasts now move units; bolt kills move corpses - positions feed the hash).
- **Tests to write first** (PS2 rows): the impulse vector of a soldier at rest (9.13, 10.01, 0.76) at (2.01 m, level);
  the flight (one update without gravity, 44 at g 13.5, down on update 46 after 14.06 m, tumbling on update 47); the
  tumble's 8.4 and the get-up's 16.8 m/s2; the 1.9333 s lock; a
  corpse's push + kick (-14.91, 7.22, 0.00) and its 4.0 s; the control outside the radius; BF2: living 0.00, the
  blast-killed thrown, the corpse; a forced-state subject unpushed.

## 9. Checks, the workflow verifier, what stayed [H]

Checks (`lead_tmp/knock/checks_fn.txt`, `checks.txt`): `re_check.py fn` on all 158 cited lines (each in the function
named); `absent` + `hashloc.py` for the PushDeadOnly hash in BF1 (0 hits; BF2 1); `word` for 2.0, 5.0, 0.6, 0.75 and
the six vtable slots; `writers` on DAT_00517874 / 78 / a4 / a8 / ac (none); `callers` + `refs.py` on the push, kick and
state-5 entry points; `addrscan.py` on 0x66eee0 (one store, the static zero).
**Verifier workflow** `knock-re-verify`: 12 agents (effort medium) over 55 claims in 8 batches, claims and citations
only (`lead_tmp/knock/claims.md`); every absence claim (6) and every BF2 twin claim (18) went to two separate
verifiers; raw verdicts in `lead_tmp/knock/verify/VERDICTS.md`. **55 claims, 79 verdicts.** Single-verified (31): 25
SUPPORTED, 6 PARTLY. Double-verified (24): 18 SUPPORTED by both; 2 SUPPORTED by one and PARTLY by the other (T10, T11);
3 PARTLY by both (C03, T16, T19); **1 NOT SUPPORTED by both (T08)**. The second verifier differed from the first on 2
of 24.
- **T08, refuted by both** from a raw scan of the disc: "no class authors PushDeadOnly 0" - two BF2 classes do (§7).
  The cited tool reads the baked classes only. Reproduced by the lead; corrected in §0, §4, §7 and the port section.
- No PARTLY contradicts a number, an offset, a state or a compare; each names an omission or a wording, now in the
  text: the push does read the health, for the -4.0 only (C03); FUN_001bbb08's callback and its network clamp (C05);
  the state setter's zoom preamble and its two extra stores (C09, T16); "off-line" is the flags' default, not a
  client test (C18); a second network gate before the chunks (C21); 0x66eee0's stores are proven for direct stores
  only (C24); a fourth condition for the rest pose (C27); BF2's kick also shakes a human below -100 health and its
  mounted branch has an extra call (T10); BF2's death block has more gates, a hide call and its death call at
  B2:159669 (T11); a BF2 citation range ended mid-function (T19).
- By the workflow's rule T10 and T11 (one of two verifiers) lose the [O]: both verifiers confirmed every number in
  them (5.0, x 9, the 0.3 + 0.4, the skip set, the scale law); what one did not accept was "and nothing else".
- Not sent to the verifiers: the measurements of §6 ([M]: the rows are in `out/`), and the BF2 flag run made after
  the verdicts (it tests T06 / T07's bit, which both verifiers supported).
**[H]:** which seats give state 13 against 12; the mount predicate that suppresses the x 9 throw; the human-only cue
on the thrown branch; E+0x4ac and BF2's flinch (E+0x4e4); the BF2 classes that die into state 0x10; BF2's other
callers of the push apply (what else knocks a living BF2 soldier down); the thrown state's air control and the
70 % draw (read, not measured); frames of the clips.
