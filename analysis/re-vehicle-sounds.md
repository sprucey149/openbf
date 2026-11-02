# OPUS RE (2026-10-03) - [O] UNAUDITED

# Vehicle sounds: the engine loop, walker steps, turret whir, boarding, collision (card VSND)

What makes a vehicle's sounds on the PS2 and what drives them, both games: the `soundparameterized` record behind
`EngineSound`, the component that feeds it, the walker's footsteps, the turret's whir and activate sound, the
collision sound. Tags: [V] read (lines cited, function checked with `re_check.py fn`), [L] measured (disc, RAM, a
live PCSX2 session), [D] derived, [H] hypothesis. Corpora: B1 `analysis/SWBFRONT.ELF-decomp.c`, B2
`analysis/SWBFRNT2.ELF-decomp.c`. Evidence, tools and the pasted checks: `engine/artifacts/lead_tmp/vsnd/`
(`claims.md`, `checks/`, `VERDICTS.md`, the two readers' notes `AGENT_B2_ENGINE.md` and `AGENT_STEPS_TURRET.md`).

## 0. Result

1. **`EngineSound`** `[O]` names a `soundparameterized` record: a `soundlayered` of 1 - 4 looping sounds plus, per
   layer, graphs `ParameterGraph(parameter, graph, target)`. A graph is a list of `ControlPoint(x, y)`, read linearly
   between points and held flat outside them. Every graph of a layer that targets Gain multiplies into one gain
   factor, every Pitch graph into one pitch factor; each sound update the layer's voice gets gain = clamp(property
   gain x factor, 0, 1) and pitch = max(0.1, property pitch x factor). No smoothing in the player.
2. **The engine component** `[O]` (one per hover, flyer, walker, droideka and soldier-with-jet-pack) sends up to 11
   parameters (BF2 12) every update of its owner. The discs use six: `speed` -> Pitch, `acceleration` -> Gain,
   `health` -> Gain (a damage layer on speeders), `turnontime` and `turnofftime` -> Gain and Pitch (the fade in and
   the run-down), and BF2's `proximity` on the guided rocket alone.
3. **On and off** `[O]`: the loop is on while the pilot seat holds a pilot. Boarding plays `TurnOnSound` and starts the
   layers (each after its `Layer` delay: 0 on the tanks, 0.8 s on the AT-ST); the gain rises over the `turnontime`
   graph (2 s on BF1's AAT). When the pilot leaves, `TurningOffSound` plays, the layers run down over the
   `turnofftime` graph, and after `TurnOffTime` seconds (3.0 unless the class sets it) they stop and `TurnOffSound`
   plays. A pilot who boards during the run-down starts the turn-on again from 0.
4. **What `speed` and `acceleration` are differs by class** `[O]`: a hover sends |forward speed| / `ForwardSpeed`
   (/ `ReverseSpeed` whenever the throttle is not forward) and a 1 %-per-update filter of its rate of change; a flyer
   |velocity| / `MaxSpeed`; a walker its speed in m/s (so its pitch graph sits at the top above 1 m/s) and - by an
   operator-precedence slip in the original, kept in BF2 - `previous speed - speed / dt` as acceleration, which reads
   the gain graph at its left end whenever the walker moves.
5. **Live** `[L]`: the model reproduces the PS2's own multipliers in 19,374 of 19,419 layer-ticks (99.8 %, the rest
   are samples taken mid-update) of three hover classes over two BF1 sessions, including a turn-off (stop at timer
   3.003 with `TurnOffTime` 3.0) and a turn-on; a BF2 state holds the walker's values.
6. **Walker steps** `[O]`: no animation event. Every non-idle update the walker measures each foot prim's underside
   height above its own origin and plays `FootstepSound<i>` (a one-shot at the foot's x / z and the walker's y) when
   the foot comes down through a line just above the lowest height that foot has had (BF1 0.05 m, BF2
   `StompThreshold`, 0.15) - or, for `StompDetectionType 1` (spider, AT-AT, AT-TE), when its descent slows below
   0.1 m per update. `HydraulicSound` (0x069134E5, `atst_leg_up`) plays once per upswing above
   `HydraulicSoundHeight`.
7. **Turret** `[O]`: `TurretYawSound` / `TurretPitchSound` are looping voices on the turret, started when the axis
   angle changes; pitch runs from `Turret*SoundPitch` x base at rest to base at the maximum rate (the key's default
   is 0.8; every BF1 class sets 0.7), gain = rate / maximum rate x the property's gain; when the angle stops the gain
   falls x0.6 per update and the voice stops below 0.0001. `TurretActivateSound` (`vehicle_equip`) is a one-shot
   when a turret seat gets a pilot, AI included; `TurretDeactivateSound` when it empties (authored by BF2 only).
8. **Collision** `[O]`: `VehicleCollisionSound` is a one-shot at the vehicle. Hover: only with collision damage.
   Walker: any closing contact with another vehicle. The sound's own `PlayInterval` (0.1 s) is the only rate limit.
9. **Port** `[V port]` (main ff13dba): none of this plays, and a seat gun's shot plays the RIDER's infantry
   `FireSound` (`game/src/event_sounds.rs` 27-29 and 95-99 look the unit's own weapon class up;
   `core/src/sim/guns.rs` 347-351 pushes `Event::Shot` with no weapon class).

## 1. The `soundparameterized` record and its graphs

1. **[L] Disc** (`vsnd/param.py`, my reader over the `snd_` chunks). A record of kind 0x12EBD555:
   `Name(<name>)`, `SoundLayered(<layered record>)`, and one scope per layer: `Layer { Name(<layer name>),
   ParameterGraph(<parameter>, <graph>, <target>) ... }`. A graph is its own record, kind `ParameterGraph`
   0xE657A914: `Name`, then `ControlPoint(x, y)` entries (0x415B9110). BF1 has 23 record names, BF2 29; a record has
   1 - 4 layers (most have three or four; 124 / 188 records over all sub-levels). Every (parameter, target) pair on
   the two discs (BF1 / BF2 counts): `speed` -> Pitch 276 / 367, `acceleration` -> Gain 261 / 349,
   `turnontime` -> Gain 228 / 299 and Pitch 168 / 168, `turnofftime` the same, `health` -> Gain 16 / 32, `speed` ->
   Gain 15 / 18, and BF2 only 0xBB0169A2 `proximity` -> Pitch and Gain (38 each, one record,
   `rep_inf_grocket_engine_parameterized`). No record uses `boost`, `height`, `strain`, `revs`, `winduptime` or
   `winddowntime`. Every graph a record names exists and has non-descending x (2 - 8 points; one BF2 graph repeats an
   x), and every graph and layered record is defined earlier in its sub-level than the record that names it - needed,
   because both are looked up while the record is parsed (1.2, 1.3). The verifier's own reader gives the same counts.
2. **[V] Loader.** B1 `FUN_004490a0` (the `snd_` dispatcher) calls `FUN_00460b80` for kind 0x12ebd555
   (507600-507601); that makes a record and parses it with `FUN_00460978` (523925-523927): `Name` 0x8d39bde6 ->
   record+4 (523841-523843); `Layer` 0x7a640f6 -> a new layer node on the list at record+8, its scope parsed by
   `FUN_004606e0` (523848-523864); `SoundLayered` 0xa54a2138 -> record+0xc = the layered record of that name, looked
   up at parse time (`FUN_0045b9e0`, 523870-523873).
3. **[V] A layer scope** (`FUN_004606e0`, 523688): `Name` -> layer[0] (523708-523710); `ParameterGraph` 0xe657a914
   -> a node {graph, parameter, target}: value 1 the parameter name hash (node[1], 523718-523719), value 2 the graph's
   name, resolved at once with `FUN_004601f0` (node[0], 523720-523722), value 3 the target (node[2], 523723-523725).
4. **[V] A graph** (`FUN_00460048` 523373 -> `FUN_0045fb08` 523121): `ControlPoint` 0x415b9110 reads two floats
   into a node {x, link, y} (523147-523154), appended in file order (523156-523161; a sorted insert when bit 0 of
   graph+8 is set, 523163-523203); `Name` -> graph+0xc (523209-523211). **Evaluation** `FUN_0045fd50(x, graph)`
   (523229): walk the points to the first whose x is greater than the argument (523271); before the first point ->
   that point's y (523275-523276); otherwise linear between the point before and it, `t = (x_i - x) / (x_i -
   x_prev)`, `y = y_prev t + y_i (1 - t)` (523279-523280); past the last point -> the last y (523253-523254).
5. **[V] Targets.** A target is one of 11 sound property fields, by a table of (setter, hash) pairs at 0x6754f8
   filled by `FUN_0045b220` (519988-520012) and searched by `FUN_00459fc0` (519188-519199): 0 `Gain` 0x1b5426fe,
   1 `Pitch` 0xbd324ab1, 2 `ReverbGain` 0x6c408cfe, 3 `Pan` 0x4f55eeec, 4 `MinDistance`, 5 `MuteDistance`,
   6 `MaxDistance`, 7 `RollOff`, 8 0x75413203 (`Trigger` by hash; the apply step skips it), 9 `RollIn`,
   10 `RollInDistance`.

Examples (`param.py bf1 NAB --sub nab1cw aat`; `bf1 END atst`): the AAT - three layers (min distance 35 / 30 / 25 m,
max 450, rolloff 2 / 3 / 4, gain 0.9, delay 0), layer 1: `speed` -> Pitch (0, 0.8) (0.5, 0.9) (1, 1);
`acceleration` -> Gain (-1, 1) (0, 0.7) (1, 1); `turnontime` -> Gain (0, 0) (2, 1) and Pitch (0, 0.5) (3, 1);
`turnofftime` -> Gain (0, 1) (3, 0) and Pitch (0, 1) (2, 0.6) (3, 0.3). The AT-ST - three layers, delay 0.8 s, 15 -
100 m (BF2 Utapau 15 - 75 m, pitch 1.5), layer 1: `speed` -> Pitch (-0.1, 1) (0, 0.8) (0.5, 0.9) (1, 1);
`acceleration` -> Gain (-0.1, 1) (0, 8) (1, 1); `turnontime` 1 s, `turnofftime` 1 s. The speeder bike: a fourth layer
with `health` -> Gain (0.1, 2) (0.3, 2) (0.4, 1.5) (0.5, 1) (0.85, 0) (1, 0) - silent above 85 % health.

## 2. Playing it: one voice per layer, a multiplier per target

1. **[V] The player** (B1 init `FUN_00460e88` 524040) binds a record with `FUN_00460f80` (524081):
   `FUN_0045c3c0(player, the record's layered record, ..)` (524092) makes one voice object per layer of the LAYERED
   record, in its order (520758-520784), each holding its layer's sound property at voice+0x110 (520777-520778); the
   record is kept at player+0x1c only when the layered record was found and every voice was made (524093-524094). A
   layer's voice is found by the layer NAME (`FUN_0045c500` 520793: the layered record's list and the voice list
   walked in step) - a graph on a name the layered record does not have reaches no voice.
2. **[V] Per owner update**: `FUN_00461000(player)` (524103) sets the 11 multipliers of every layer voice
   (voice+0x130 + 4 i) to 1.0 (524122-524130). `FUN_004610b8(value, player, parameter, direct)` (524139): for every
   layer, every graph node of that parameter (524170): `direct == 0` and the node has a graph -> multiplier[target]
   x= graph(value) (524173-524177); `direct != 0` -> multiplier[target] x= value (524181-524182). So several graphs on
   one target multiply.
3. **[V] Per sound update** (`FUN_00448ed8`, only when dt > 0, 507492; `FUN_00461508(dt)` 507529 -> every player:
   `… then, when the player is
   playing (524209), for every layer voice: gain = clamp(property gain (P+0x14) x multiplier[Gain], 0, 1)
   (524220-524225), pitch = max(0.1, property pitch (P+0x10) x multiplier[Pitch]) (524226-524233), and the other
   fields the same way (524234-524296: reverb gain, pan in -1..1, min <= mute <= max distance, rolloff, roll-in).
   [D] The property's `PitchDev` / `GainDev`, drawn when a layer voice starts (`FUN_00459810` 518880-518885), are
   overwritten here on the next update.
4. **[V] Layer starts** (`FUN_0045c700(dt, player)`, 520888): a pending request changes the state - play: elapsed
   = 0, started = 0 (520941-520944); stop: every voice stopped (520923-520937, 520950-520964). While playing and not
   every layer has started (520974): a layer voice that is not playing starts when `delay + 2 x deviation x (r -
   0.5) <= elapsed`, r uniform, redrawn each update (520990-520996; the LAYERED record's delay and deviation,
   `re-weapon-sounds.md` 1.2); then elapsed += dt (521000).

## 3. The engine component

1. **[V] Class keys** - a block inside the class, set by `FUN_00366ea0` (354600), which five class setters call:
   hover `FUN_001f0648` (102469), flyer `FUN_001df3f0` (90095), walker `FUN_0021d980` (133978) with class+0x1730;
   droideka `FUN_001d3f68` (82046) and soldier `FUN_0020f2e0` (123089) with class+0x16f0.

   | key | hash | block offset | what |
   |---|---|---|---|
   | `EngineSound` | 0x84bbef6a | +0 | the parameterized record, by name (`FUN_00365818` 353769 -> `FUN_00460d28`: list 0x675fb8, 523988) |
   | `TurnOnSound` | 0xea603e52 | +4 | sound reference (354661-354662) |
   | `WindUpSound` 0x205b8d59, `TakeoffSound` 0xc093b232 | | +0xc | one slot (354641-354645, 354654-354657) |
   | `LandSound` 0xad3e89e3, `WindDownSound` 0xfc64a1f2 | | +0x14 | one slot (354653, 354665-354669) |
   | `TurningOffSound` | 0x7d2c2fa2 | +0x1c | (354626-354627) |
   | `TurnOffSound` | 0x42dabae | +0x24 | (354637-354638) |
   | `TurnOffTime` | 0xc8a9e3c6 | +0x2c | float `"%f"` (354648-354649); 3.0 from the constructor `FUN_00366cb8` (354551) |
   | `BoostSound` | 0x79e2d013 | +0x30 | a list; `"%s %f %d"`: sound, threshold, rising flag (354617-354622) |
   | `VehicleCollisionSound` | 0x84824518 | +0x34 | sound reference (354630-354633) |

   [L] The names of the hashes `audio-census.md` listed raw: 0xEA603E52 `TurnOnSound`, 0x042DABAE `TurnOffSound`,
   and by the same hash 0x7D2C2FA2 `TurningOffSound`, 0x205B8D59 `WindUpSound`, 0xFC64A1F2 `WindDownSound`.
2. **[V] Instance** (`FUN_003659b0` 353859; at entity+0x59c for hover / flyer / walker, +0x568 for the droideka
   and the soldier; every instance is on the list 0x647b50, 353901-353906): +4 the player, +0x24 a sound handle and
   +0x28 its reference (the turn-on / turn-off one-shots), +0x30 / +0x34 a second handle (boost), +0x3c the previous
   `speed`, +0x40 a timer, +0x44 a u16 mask - bit i set when the record has a graph for parameter i of the table at
   0x547e00 (353885-353895) - and +0x46 flags: 1 = has a record (353899), 2 = on, 4 = wind-up, 8 = wind-down, 0x10 =
   turning off (`FUN_00366550` 354210 and the three after it). The table [ELF words]: 0 `speed` 0x7b80c780, 1
   `acceleration` 0x2fa0ba9d, 2 `health` 0x6b98ed8f, 3 `boost` 0xf8e9242c, 4 `revs` 0xd7c3db73, 5 `height`
   0xd5bdbb42, 6 `turnontime` 0x08aab3aa, 7 `turnofftime` 0xc8a9e3c6, 8 `strain` 0xc7ab9270, 9 `winduptime`
   0x43f6a333, 10 `winddowntime` 0xeecf1cca.
3. **[V] Turn on** `FUN_003660b8(health, instance, class, pos, vel)` (354067): when the instance is off, or is
   turning off (354077): `FUN_00366430(.., class+4, .., 2)` (354078) - `TurnOnSound` on the handle (354177-354179),
   timer = 0 (354180), flags = on with the three transient states cleared (354181); the player is asked to play
   (354081); one update with dt 0, speed 0, acceleration 0 and that health (354085).
4. **[V] Update** `FUN_00365bc0(dt, speed, acceleration, health, boost, revs, height, strain, instance, class, pos,
   vel)` (353939): the handle follows the position (353954); nothing more unless on (353955-353957) and a record is
   bound (353960); multipliers reset (353963); each parameter whose mask bit is set is sent through its graph -
   speed, acceleration, health, boost, revs, height (bits 0 - 5, 353964-353981) and strain (bit 8, 353982-353984);
   the layer voices take the position and velocity pointers (353985-353986); the boost list (3.6); previous speed =
   speed (353989). Then the timer feeds ONE of the four time parameters through its graph and the other three get
   the value 1.0 directly: turning off -> `turnofftime` (353997-353999) and the turn-off check (354006); wind-down ->
   `winddowntime` (354035-354036); wind-up -> `winduptime` (354049-354050); otherwise -> `turnontime`
   (354012-354013). timer += dt (354060). Only flyers enter wind-up and wind-down (`FUN_003661b0` at take-off,
   `FUN_00366228` at landing, two `jal`s each, all in the flyer); a flyer stays in wind-up for the rest of its flight,
   so its `turnontime` graphs read 1.0 after take-off. Every entry point of the component first asks the gate
   `FUN_00365898` (353949; session flags, 1 offline [H]).
5. **[V] Turn off** `FUN_00366318(instance, class, pos, vel)` (354138): on and not yet turning off -> `FUN_003662a0`
   (354147-354148): `TurningOffSound` on the handle, timer = 0, state = turning off (354130; 354180-354181). On and
   `TurnOffTime <= timer` (354153): `TurnOffSound` on the handle (354154-354156), the player is asked to stop
   (354158), on and turning-off cleared (354160). `FUN_003664e8` (354189) stops at once (354196-354199);
   `FUN_00366730` (354306) does that and unbinds the record (354317-354323).
6. **[V] `BoostSound`** (`FUN_00367070` 354679 -> `FUN_00366b08` 354472, per list entry): with the rising flag the
   entry's sound is played on the second handle when `previous speed <= threshold < speed` (354485-354486); without
   it when `speed < threshold <= previous speed` and the handle is not playing (354481-354482, 354492-354494). [L]
   no BF1 class has the key; 96 BF2 class chunks do, all flyers (`..._shift_up_property 0.11 1`).

## 4. What each class feeds (BF1)

Six `jal`s to the update exist in the ELF (0x1d1de4, 0x1db814, 0x1edbf8, 0x207388, 0x21a9c0 and the turn-on's own
0x366184; `scripts/tools/refs.py`): the five class updates below.

| class (B1 function) | on while | `speed` | `acceleration` | `health` | `height` | `strain` |
|---|---|---|---|---|---|---|
| hover (`FUN_001ea9f8` 97428-97461; `FUN_001ec768` 99513-99566) | the pilot seat has a pilot (entity+0x384) | \|forward axis . velocity\| / S, S = `ForwardSpeed` when the throttle is > 0, else `ReverseSpeed` | A += (a - A) x 0.01 per update, a = (speed - previous speed) / dt | health / max health | the ground factor (entity+0x610) | T += (t - T) x 0.1 per update; t = \|throttle - signed speed\| when the vehicle is not following the throttle, else 0 |
| flyer (`FUN_001d9180` 86581-86649) | pilot (turned off only in flight state 0) | \|velocity\| / `MaxSpeed` | ((speed - previous) / dt) / `Acceleraton` | same | 0 | entity+0x2110 |
| walker (`FUN_00219ed8` 130843-130891) | pilot | \|velocity\| in m/s, not normalised | previous speed - speed / dt | same | 0 | 0 |
| droideka (`FUN_001d1540` 79849-79897) | state 0xb or 0xc (the ball) | \|velocity\| m/s | previous speed - speed / dt | same | 0 | 0 |
| soldier (`FUN_00209540` 118809-118822; `FUN_00205cc8` 117352-117376, 117778-117783; a second turn-off in `FUN_002012c8` 113045-113059) | state 7 (jet flight) | \|velocity\| m/s | previous speed - speed / dt | same | 0 | 0 |

`boost` and `revs` are 0 at every BF1 call site.

- **[V] Hover**, from the ELF (0x1eda64-0x1edbf8, `checks/dis_b1_callsites.txt`): the block runs only when dt > 0;
  the dot product is 3-D (`VMUL.xyz`) of entity+0x220 and the velocity entity+0x5f0; S = class+0x1788 when the
  throttle (the function's second argument, entity[0xc0], 97698) is > 0, else class+0x178c, and 1.0 when that is 0;
  signed speed n = dot / S, demand m = throttle x S / S; speed = |n|. t = |m - n| when (a <= 0 and n < m) or (a >= 0
  and m < n), else 0 (0x1edb08-0x1edb50). The two filters use 0.1 (0x3dcccccd) and 0.01 (0x3c23d70a) per update, not
  per second (the PS2 runs 30 updates a second, `a5-tickrate.md`).
- **[V] The walker, the droideka and the soldier** pass `previous - speed / dt`: `div.s $f1, $f20, $f22` then
  `sub.s $f14, $f0, $f1` (0x21a964-0x21a988; the same pair at 0x1d1d88-0x1d1dac and 0x207328-0x207348). The flyer and
  the hover divide the difference. At 30 updates a second a walker at 6.5 m/s sends about -188, at rest 0.
- **[V] On while a pilot sits in the pilot seat**: hover 97430-97437 (no pilot and on -> turn off), 97441-97455
  (pilot and off -> turn on) and 97458-97461 (pilot and turning off -> turn on); walker 130843-130866; flyer
  86590-86608 (its turn-off also needs flight state entity[0x186] == 0, 86594; a flyer in any other flight state
  that is not on is turned on with no pilot test, 85415-85432, and the carrier subclass turns on in its
  constructor, `FUN_001c5d40` 71105). entity+0x384 is the pilot of the vehicle's own seat: the seat keeps its
  pilot at +0xe4 (`FUN_0023c488` 154789, 154801) and sits at entity+0x2a0 - the verifier closed that for the hover
  (the seat's vtable word 0x531468 is -0x2a0 and its slot +0x68, `FUN_001f2408`, calls `FUN_001ef4c0(seat - 0x2a0)`,
  which reads `*(entity + 900)`, 100726-100730). [L] the live run: the word going to 0 starts the turn-off within
  one update (section 6).
- **[L] Which classes author what** (the whole disc, `verify/propscan.py`, re-run): BF1 `TurnOnSound` /
  `TurnOffSound` 6 class chunks each - the jet pack (3), the tauntaun (`tauntaunstart` / `tauntaunstop`), the
  droideka (`cis_droideka_fold` / `_unfold`), the kaadu (`kaadu`); no tank, speeder or AT-ST. `TurnOffTime` 10 chunks
  (0.0 x 3, 5.0 x 3, 2.0 x 2, 1.0 x 2), the rest the default 3.0; `TakeoffSound` 2 (snowspeeder, gunship);
  `TurningOffSound`, `BoostSound`, `WindUpSound`, `WindDownSound` none. BF2: `TurnOnSound` 59 (27 of them
  `com_inf_saber_on` on a weapon class - another reader, not opened), `TurningOffSound` 18 and `BoostSound` 96
  (flyers), `TurnOffTime` 32 (1.5 x 17, 0.0 x 9, 5.0 x 4). The kaadu and the tauntaun have the two sounds and no
  `EngineSound`: the turn-on and turn-off still run for an instance without a record (the sounds are on the handle,
  354177-354179, 354154-354156).

## 5. BF2

Read by a second reader (`AGENT_B2_ENGINE.md`); every line cited here was opened again.

1. **[V] The same component**, 0x44c368-0x44ddd8: setter `FUN_0044da60` (510559; `EngineSound` 510570), update
   `FUN_0044c650` (509866), turn on `FUN_0044cc28`, turn off `FUN_0044ce98`, collision `FUN_0044d3b8`. The class
   block is at class+0x600 (hover, flyer, walker) / +0x5b0 (droideka, soldier); the instance at entity+0x464 on the
   hover and the walker. Parameter table 0x6392c0: BF1's 11 words and a 12th, `proximity` 0xbb0169a2 [ELF words].
2. **[V] Additions**: four keys - `Cockpit1stPersonSound` 0x1fe0c822 (+0x3c), `Cockpit3rdPersonSound` 0x421337df
   (+0x44), `ProximityMinDist` 0xf3a06dca (+0x4c, 60.0) and `ProximityMaxDist` 0xac4226d4 (+0x50, 500.0; the
   constructor `FUN_0044d7e8` 510500, 510504-510505); the update takes two more integers and a distance: a cockpit
   one-shot asked for on every update while the first is set (509889-509895), `proximity` = 1 - (clamp(d, min, max)
   - min) / (max - min) (509931-509943), and a return when no record is bound (509906). The walker, the flyer and the
   soldier pass the first integer as 1 on every update while a local human controls the unit; only the flyer passes
   a distance (the others 0.0, so their `proximity` is 1.0). [L] the cockpit keys carry the space chatter (19 class
   chunks); `proximity` is used by the guided rocket's record alone.
3. **[V] Hover**: as BF1 - S = class+0x670 / +0x674 by the throttle's sign (134616-134621), speed = |n| (134629),
   a = (speed - previous) / dt (134630), strain filter 0.1 (134645), acceleration filter 0.01 (134646-134647), the
   call 134660, previous = speed (134665).
4. **[V] Walker**: speed = |entity+0x2240| in m/s (180039-180048), acceleration `previous - speed / dt` (180112;
   `div.s` then `sub.s`, 0x260608 / 0x260638), turn on with a pilot and not on (180025), turn off when on with no
   pilot (180033).
5. **[V] Droideka** (differs from BF1): speed = |velocity| / class+0xe8c, falling no faster than 4 / s; acceleration
   = (old - new) / dt; off the ground speed eases to 0.5 and acceleration is 0 (109569-109604).
6. **[V] The player**: the same multipliers and clamps (set `FUN_005536c8` 686677-686686; apply `FUN_00553838`:
   gain 686730-686736, pitch 686737-686744; graph `FUN_005526f0` 685863). Two differences: each layer's start time
   `delay + 2 x deviation x (r - 0.5)` is drawn once when the play request is taken (`FUN_0054f5f8` 683836-683841)
   and a layer that is not playing after its start time is started on any later update (683886-683888); and a record
   with a `time` graph gets the player's own play time (686716-686718; no disc record has one). BF2 also draws an end
   time per layer from a `Layer` entry's 5th / 6th values (683842-683848), which no disc entry has
   (`re-weapon-sounds.md` 1.4).

## 6. Live evidence [L]

Tools: `vsnd/ram_engine.py` (BF1: walks the instance list 0x647b50 in a `.p2s` or a live session; `--watch` samples
every instance per game-clock tick, `--pad` / `--hold0` script the session), `vsnd/ram_engine2.py` (BF2, list
0x71fa48), `vsnd/replay.py` (the model over a watch csv).

1. **A state** (`lead_tmp/sqveh/nab1_cis_aat_pilot.p2s`, BF1 NAB1): the AAT at rest - mask 0x0c3 (speed,
   acceleration, turnontime, turnofftime), the three layers' voices at pitch 0.8000 / 0.8500 / 0.6000 and gain 0.6300
   / 0.5400 / 0.7200 = the graphs at speed 0 and acceleration 0 x the property's gain 0.9. A STAP at 20.005 m/s
   (`ForwardSpeed` 20): stored speed 1.0002, pitch 1.0000 on all three layers, gain 0.5994 = 0.9 x (0.6 + 0.6 x
   0.11004), the stored acceleration being 0.11004. A fighter tank at 5.719 m/s of `ForwardSpeed` 12: 0.4766.
2. **Replay** (`run_live.sh` -> `live1.csv`, 32 game s, the player driving an AAT; `run_live2.sh` -> `live2.csv`):
   for every sampled tick of every hover instance the Gain and Pitch multipliers predicted from the disc's graphs and
   the vehicle's own stored speed, acceleration and timer against the multipliers in memory, with one fixed rule
   for the timer (the sampled timer - 1 / 30; the verifier's `verify/strict_replay.py`, re-run): AAT 3940 / 3957,
   STAP 6013 / 6027, fighter tank 9421 / 9435 layer-ticks within 0.002. My own `replay.py` tried four timer
   candidates per row and kept the nearest (3951 / 6016 / 9421) - that looks at the answer and is not the number to
   quote. The misses are samples between the reset and the last parameter, or a tick whose dt was not 1 / 30.
3. **The normaliser**: fwd -9.729 m/s -> stored 0.9728 (/ `ReverseSpeed` 10); 11.885 m/s forward -> 1.1885 (/ 10,
   not / 12 [D: the throttle no longer forward]); 12.024 m/s -> 1.0020 (/ 12).
4. **The filter**: the stored acceleration decays -0.12577 -> -0.10818 in 0.5 s = x0.860 = 0.99^15.
5. **Turn-off and turn-on** (`live2.csv`: the AAT's pilot word held at 0 from game second 2 to 9): at 2.07 the flags
   go 3 -> 0x13 with the timer at 0.0667; the gain multiplier of layer 1 falls 0.69 -> 0.39 (timer 1.37) -> 0.007
   (3.00) and the pitch 0.79 -> 0.59 -> 0.25; at timer 3.003 the flags are 1 (the hover's own turn-off call takes
   it, before the update, so the last multipliers are those of timer 2.97) and one tick later the three voices are
   idle. With the pilot back at 9.01: flags 3, timer 0, gain multiplier 0.012 -> 0.70 at timer 2.2 (the 2 s
   fade), pitch multiplier 0.41 -> 0.80 at timer 3.3. A STAP boarded by a bot (`live1.csv` 14.55 s) shows the same
   from timer 0.
6. **Droideka** (`live1.csv`): flags 3 -> 0x13 -> 1 within 0.1 s, the voice idle one tick later.
7. **BF2** (`ram_engine2.py` on `ref/states/bf2_uta1_cp3_con_cp1_postA_spawn.p2s`): two AT-RTs walking at 5.58 and
   5.57 m/s (stored speed 5.5845, 5.5654 - m/s): Pitch multiplier 1.0000 on all three layers, Gain multipliers 0.7
   / 1.0 / 1.0 = the three `acceleration` graphs at their left ends ((-0.1, 0.7), (-0.1, 1), (0, 1)); each layer's
   start time 0.800.

## 7. Walker footsteps

Read by the second reader (`AGENT_STEPS_TURRET.md`); B1 `FUN_00216b30` was read again in full, B2's rule lines opened.

1. **[V] Keys** (walker class setter B1 `FUN_0021d980`, B2 `FUN_00263f68`): `FootstepSound0..5` -> class+0x28a0 +
   8 i (B1 133766-133768, 133755, 133734) / +0xe80 + 8 i (B2 183573); `HydraulicSound` 0x069134e5 -> +0x176c (133292)
   / +0xe68 (182975); `HydraulicSoundHeight` 0x8ae91462 -> +0x1774 (133665; 1.0 from the constructor, 132801);
   `HydraulicLowerSound` 0x0379fd4a / `HydraulicLowerHeight` 0x08080fd4 (authored by no class); the foot list: B1
   every `TerrainCollision` line in ODF order (134018), B2 every `TerrainLeft` / `TerrainRight` line; BF2
   `StompThreshold` 0x4f08e1f5 -> +0xe34 (183281; 0.15 from the constructor, 182439). [L] 0x069134E5 was a raw hash
   in the census.
2. **[V] When it runs**: the locomotion calls the step function on every update whose animation is not the idle one
   and zeroes the latch word on an idle update (B1 `FUN_00218fa0` 129704-129726; B2 `FUN_0025d5e0` 178381).
3. **[V] The rule** (B1 `FUN_00216b30` 128061; B2 `FUN_0025aaa0` 176065), per foot i of the list: h = the foot
   prim's underside (prim position - its up axis x half its size) minus the walker's own y; prev = h of the update
   before; low = the lowest h this foot has had (FLT_MAX at first).
   - `StompDetectionType` 0 - B1: step when `h < low + 0.05` and `low + 0.05 < prev` (128139-128144); B2: `h <
     prev`, `h < low + StompThreshold` and `low + StompThreshold < prev` (176135-176142).
   - `StompDetectionType` not 0 - a latch per foot: clear and `prev - h < 0.1` -> set, step; set and `prev - h >
     0.1` -> clear (128146-128158; B2 176145-176157). Per update, not per second.
   - then low = min(low, h) (128163-128164), prev = h (128196).
   - `HydraulicSound`: while h rises, once when `h > HydraulicSoundHeight` (latched until the foot stops rising), a
     one-shot at the foot's underside with the walker's velocity (128170-128181; B2 176168-176173).
   - `HydraulicLowerSound`: the mirror on the way down, below `HydraulicLowerHeight` (128183-128195); no class on
     either disc sets it.
   - a step: rumble for a human occupant (128198-128211), a water ripple, a one-shot from the collider's
     `FoleyFXWalker` set when it has one (128220-128223; none ships, `re-foley.md`), then a one-shot of
     `FootstepSound<i>` at (foot x, the walker's y, foot z) with no velocity (128212, 128224; B2 176237), and the
     class's `StompEffect`.
4. **[L] Which index is which foot** (the baked classes): AT-ST 0 `p_l_foot`, 1 `p_r_foot`, both
   `com_weap_layered_at_step`, `HydraulicSound` `atst_leg_up` at 0.2; BF1 kaadu and tauntaun: the list is
   `p_collision`, `p_l_foot`, `p_r_foot` and the sounds are `FootstepSound1` (left) and `FootstepSound2` (right) -
   index 0, the body, is silent; BF2 tauntaun 0 left, 1 right, `StompThreshold` 0.5; spider and AT-AT four feet,
   type 1; the AT-TE authors slots 0, 1, 4, 5 for four feet [D: its rear feet are silent - not heard live].
5. **[L] Rates** (SNDC's logs, starts chained to the previous start within 8 m and 2 s): BF2 UTA1 AT-RT
   `com_weap_layered_at_step` 136 starts, the gap between one walker's steps 0.33 s (54 of 123) or 0.5 s (28), median
   0.334; `atst_leg_up` 104 starts; BF1 NAB1 kaadu steps 77 starts, gap 0.33 s in 45 of 59.
6. **[V] The droideka** has its own three keys (`Footstep0Sound` 0x21f56ed0 .. `Footstep2Sound`) and plays them when
   a leg bone's height falls through 0.48 (B1 `FUN_001cdd10` 76929; not re-read here).

## 8. Turret whir, activate, deactivate

1. **[V] Keys** (turret class setter B1 `FUN_00225f08`, B2 `FUN_00270770`): `TurretYawSound` 0x1fa43b99 ->
   T+0x116c (138985) / +0x850 (191603), `TurretPitchSound` 0x28f016d2 -> T+0x1178 (138998) / +0x85c (191615),
   `TurretYawSoundPitch` 0x1503f425 and `TurretPitchSoundPitch` 0x4ed7a4d0 (floats; 0.8 from the constructor,
   138900, 138903; [L] 0.7 in all 25 BF1 class chunks that have a whir, BF2 0.7 x 114 and 0.6 x 9),
   `TurretActivateSound` 0xe9d0fb69 -> T+0x1184 (139140) / +0x868 (191576), `TurretDeactivateSound`
   0xf84aa204 -> T+0x1194 (139148) / +0x878 (191582).
2. **[V] The axis function** B1 `FUN_00224f48(|rate|, max rate, sound pitch, turret, old angle as int, new angle
   as int, pos, vel, sound, handle, &base, &gain)` (138122; B2 `FUN_0026edc8` 190042), called for the pitch axis and
   then the yaw axis at the end of the turret's aim update (B1 137529; B2 189387), the angles scaled by 100 (B2
   1000) before the cast:
   - the integers equal: a playing handle's voice gain g -> g - 0.4 g while g > 0.0001, else the handle is stopped
     (138139-138160);
   - else, with max rate > 0 (138164-138166): r = |rate| / max rate; when the stored gain is below the property's
     gain it rises by 0.1 x the property's gain (138168-138171); a handle not playing is started - a looping handle
     play - and base = the property's pitch, gain = the property's gain (138177-138184), so the rise never shows
     [D]; the voice's pitch = ((1 - r) x sound pitch + r) x base, kept between sound pitch and base; its gain =
     clamp(r x gain, 0, 1) (138190-138203).
3. **[V] Activate / deactivate**: the tail of the same function (138207-138219): when the turret's pilot (turret
   +0xf4) differs from the last one seen (+0x3d4): a pilot -> a one-shot of `TurretActivateSound` (138216), none ->
   `TurretDeactivateSound` (138212), at the position passed in; no player test. BF2 the same (190126-190136). After
   it a third handle holds `TurretAmbientSound` 0x8e5da6e8 while the turret has a pilot (138222-138231; 4 BF1
   class chunks, `droid_fx7_hum`). `TurretStartSound` (16 BF1 chunks, `turret_whir_start`) is stored and played
   by nothing. The two calls sit inside a test of a flag of the turret's owner (137356-137359; cleared by the kill
   function [second reader]) - a dead owner's turret makes no sound.
4. **[L]** `vsnd/snd_board.csv` (`run_live3.sh`): the player leaves the AAT at 2.97 s - his two weapons' equip
   sounds at 3.00 and no `vehicle_equip`; at 7.98 he takes a gun turret - `vehicle_equip` at 8.01, 6.1 m from the
   eye, and `turret_whir_pitch_lp` / `turret_whir_yaw_lp` start at 8.31 / 8.38; a third `vehicle_equip` at 14.58 s
   with no press before it is another unit's. [L] The keys sit in `TURRETn` sections (BF1: the AT-STs, the fighter
   tanks, the combat speeder, the AT-TE, the gun turret; not the AAT): the driver's own seat has none. Disc:
   `TurretActivateSound` 25 BF1 class chunks; `TurretDeactivateSound` = `vehicle_equip` 92 BF2 chunks, BF1 none.

## 9. Collision

1. **[V]** `FUN_00366858(instance, class block, pos, vel)` (B1 354359; B2 `FUN_0044d3b8` 510318): a one-shot of
   block+0x34 (354368). Five `jal`s in BF1, six in BF2.
2. **[V] Hover** (B1 `FUN_001ee3e0`; B2 `FUN_0021f978` 135133): inside the collision damage branch - closing speed s
   < 0, s < -threshold, damage > 0 (99945-99961; the damage is first multiplied by a factor of the other object when
   it has one, 99955-99960) - at the vehicle's position with its velocity (99985). **Walker** (B1 `FUN_00217d50`; B2
   `FUN_0025c2b0` 177404): the other body's collision type word is 4 (vehicle; BF2 8) and s < 0 (129005, 129042),
   before the threshold test (129078-129079) - never terrain, buildings or soldiers. The droideka (`FUN_001ce828`
   77582, also before its threshold test) and the flyer (`FUN_001dd4c8` 88309, 88652): `AGENT_STEPS_TURRET.md` D.3
   (conditions not re-read).
3. **[L]** `com_veh_collision_lg` has `PlayInterval` 0.1 s (`re-weapon-sounds.md` 3.5): the property is what limits
   a contact that lasts.

## 10. PS2 | port | card

| | PS2 | port (main ff13dba) | card |
|---|---|---|---|
| seat gun's shot | the gun's `FireSound`, one per shot (`re-weapon-sounds.md` 7.5) | the rider's infantry `FireSound` | VSND |
| `soundparameterized`, `ParameterGraph` | parsed, bound per vehicle | not baked | VSND |
| engine loop | 1 - 4 looping layers per piloted vehicle, gain and pitch by graphs | nothing | VSND |
| engine on / off | pilot seat; turn-on fade; `TurnOffTime` run-down | - | VSND |
| `TurnOnSound` / `TurnOffSound` (kaadu, tauntaun cries) | on boarding / after the run-down | nothing | VSN2 |
| walker steps, `HydraulicSound` | per foot, by the height rule | nothing | VSN2 |
| turret whir | two looping voices by turn rate | nothing | VSN2 |
| `TurretActivateSound` / `Deactivate` | a turret seat gets / loses its pilot | nothing | VSN2 |
| `VehicleCollisionSound` | hover: with collision damage; walker: vehicle contact | nothing | VSN2 |
| jet pack and droideka engine, BF2 cockpit chatter, `BoostSound`, flyers' take-off / land | the same component | nothing | notes (USND, post-alpha flyers) |

### 10b. What the port does after VSND (main 79eac42, 2026-10-03)

- **Seat guns**: `Event::Shot` carries the weapon class that fired; `game/src/event_sounds.rs` plays that class's
  `FireSound`. NAB1, the 105 s pad replay: 49 rider shots moved from infantry names to five vehicle weapon names,
  1,395 shots before and after.
- **Bake**: `sounds.ron` has `engines` - one entry per `soundparameterized` record per sub-level (`level` = the
  sub-level's name hash), the layers in the layered record's order with their delay and deviation, each with the
  graphs of the `Layer` scope of its name (parameter and target as names, the points).
- **The sub-level matters** `[L]`: NAB.LVL defines `interceptorTank_engine_parameterized` in four sub-levels with
  three different sets of graphs (`nab2gcw`: `speed` -> Pitch (0, 0.5) (0.5, 0.5) (1, 1) against `nab1cw`'s (0, 0.8)
  (0.5, 0.9) (1, 1)). The battle looks a record up in `<map><era>` (`game::foley::sublevels`, the foley table's
  rule). Picking the file's last definition instead gave 19,107 of 19,419 in the replay below; the sound
  PROPERTIES are still merged that way (`game::sounds::merge_props`, PROVISIONAL).
- **Engine**: `game/src/engine_sound.rs` - the component of sections 3-4 per hover and walker (on with a pilot, the
  run-down, the hover's filtered acceleration, the walker's slip), a looping voice per layer set every tick to gain =
  clamp(property gain x factor, 0, 1) x the level by distance and rate = max(0.1, pitch x factor).
- **Referee**: the PS2's stored speed, acceleration, health, timer and flags of `live1.csv` + `live2.csv` through
  the BAKED NAB1 records and the port's `factors` (the fixed timer rule of 6.2): AAT 3940 / 3957, STAP 6013 / 6027,
  fighter tank 9421 / 9435 = 19,374 of 19,419 layer-ticks within 0.002 - the disc's own number
  (`engine_sound::tests::the_ps2s_multipliers_replay_through_the_baked_records`).
- **Not ported**: the turn-on's and turn-off's one-shots, steps, whir, collision (card VSN2); flyers, the droideka
  and the jet pack feed nothing; the property's `PitchDev` / `GainDev` and `ReverbGain` on a layer; the 39-voice
  limit (every engine inside its mute distance plays); BF2's layer start drawn once (the port redraws the deviation
  each update, BF1's rule, for both games); `boost`, `revs`, `height`, `strain` (no disc record has a graph on them).

## 11. Checks run

`engine/artifacts/lead_tmp/vsnd/checks/`: `fn_b1.txt` (136 cited B1 lines, each in the function named),
`fn_b1_agentB.txt` (48), `fn_b2.txt` (75 B2 lines), `callers_b1_engine.txt` (`re_check.py callers` of the 15 engine
entry points), `words_refs_b1.txt` (`re_check.py word b1 0x547e00 11`: the 11 parameter hashes; `0x547e38` = `"%s %f
%d"`, `0x547e48` = `"%f"`; `scripts/tools/refs.py b1`: update 6 jal, turn-on 7, turn-off 7, collision 5, stop 2, wind-up 2,
wind-down 2 - the decomp's caller counts, no data word), `dis_b1_callsites.txt` (`eedis2.py`: the hover 0x1eda60-
0x1edc08, the walker 0x21a930-0x21a9d0, the flyer, the droideka, the soldier). `re_check.py word b2 0x6392c0 12`:
BF1's 11 words + 0xbb0169a2; `refs.py b2 0x44c650`: 6 jal; `0x44da60`: 5. Names: `foly/h.py` on every name in this
note (the hash function of `re-sound-banks.md`). The verifier's own tools are in `vsnd/verify/` (`census.py`,
`strict_replay.py`, `propscan.py`).

## 12. Verifier

One foreground Opus verifier, the claims and citations only (`claims.md`, 62 claims): 53 SUPPORTED, 9 PARTLY, 0 NOT
SUPPORTED (`VERDICTS.md`). Every constant was re-read in the ELF and every caller count re-scanned (`refs.py`). The
PARTLY verdicts, all folded in: seven cited-line slips (2.3, 4, 5.2, 9.2, 0.9: the `dt > 0` test is 507492, the
hover's turn-off call 97437 and its turning-off test 97458-97461, the walker's store 130891, the seat's store 154801,
BF2's constructor lines, the walker's `s < 0` line 129042, the port's `Shot` arm 95-99 on today's main); the turret
function's gain rise that the claim skipped (8.2); and my replay, which chose among four timer candidates per row -
the fixed-rule numbers are now the ones quoted (6.2). From its "found beside the claims", folded in: the target
`Trigger`; the non-strict x order and the definition order on the disc (1.1); the layer-name lookup and the bind
condition (2.1); the gate on every entry and the flyer staying in wind-up (3.4); the flyer's second turn-on site and
the carrier (4); the hover's seat base, closed from the vtable word (4); the cockpit call on every update and the
proximity input (5.2); BF2's layer end time (5.6); the step's other one-shots and `HydraulicLowerSound` (7.3);
`TurretAmbientSound`, the unplayed `TurretStartSound`, the 0.7 `Turret*SoundPitch` on the disc, the owner flag
(8.1, 8.3); the damage factor and the collision type word (9.2); the whole-disc key census (4, 3.6, 8.4).
Reproduced by the author afterwards: `strict_replay.py` (3940 / 6013 / 9421), `propscan.py` on both discs, each
corrected line.

## 13. Not settled

- The voice level under a layer voice: which of the 39 mixed voices a layer holds, when a far engine loses its voice
  and takes it back, any ramp on a gain change. The census's "338 audible engine layer starts in 105 s" are those
  re-takes [H].
- `FUN_0018e740(player, 0)` in the turn-on and the bit 0x80 of the player's state word (354081-354083).
- The flyer's `strain` field and its flight states; the flyer block of BF2 (boost, proximity) was read by the second
  reader only.
- The gate before every engine call (`FUN_00365898` / `FUN_0044c3c8`: session flags) is described by use; offline it
  passes [H].
- Whether the AT-TE's rear feet are silent on the PS2, and a type-1 walker's first non-idle update (7.4, 7.3).
- Which seats set a turret object's pilot (the set-pilot's 39 callers were not read); the live run shows a gun
  turret, not a vehicle's gunner seat.
- The property's `PitchDev` / `GainDev` on an engine layer (2.3) rest on the order of two calls, not on a live read.
