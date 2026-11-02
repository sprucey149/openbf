# OPUS RE (loop lane 2, 2026-10-07, card VTAT) - [O] UNAUDITED: no Fable pass
# RE: turrets and the AI - who drives a turret seat, the self-piloted turret (`PilotType = self`), `AutoFire`, and how a seat's gun is triggered

**The questions** (card VTAT). (1) What is the PS2's "automatic turret" - a turret section whose ODF says
`PilotType = self` - and how does it acquire and fire? (2) What does an AI do in a turret seat? (3) The planner's:
does a seat gun follow its own trigger rule (the port's bot gunner holds the trigger)? (4) What is the "MountedTurret
acquisition" in B1 `FUN_00223850` that `re-vehicles.md` 9 and `re-aim-assist.md` 3.10 located?
Whether and when bots BOARD and LEAVE a turret is already in `re-soldier-ai.md` 7 and 10a; it is not repeated here.

**Method.** Both decomps and ELFs (lines `B1:` / `B2:`); the discs (every class and world instance, the mission
scripts' bytecode); and the PS2 itself: BF2 Hoth, 1-flag CTF, booted from Instant Action on PCSX2 (software
renderer), every AI-driven turret seat sampled from RAM over 5 minutes of mission time (three captures) and one save
state kept. BF1's code was read by a reader agent (`AGENT_BF1.md`); what I re-opened of it myself is marked. Scratch,
tools and raw rows: `engine/artifacts/lead_tmp/vtat/` (section 9). Tags: [V] read (lines cited), [D] derived, [H]
hypothesis, [L] seen on the running PS2, `[O]` load-bearing and SUPPORTED by the verifier (section 9).

Names (BF2): T = a MountedTurret (entity vtable 0x620128), C = its Controllable = T+0x10 (vtable 0x620050 at C+0x14),
K = its class `*(T+0x27c)`, owner = the building or vehicle `*(T+0x278)`; ai = the UnitController `*(C+0xb4)`,
mv = ai+0x430 its mover. BF1: T (vtable 0x5343b8), C = T+0x10 (vtable at C+0x294 = 0x534310), K = `*(T+0x39c)`,
owner E = `*(T+0x398)`.

## 0. The answers

| question | answer |
|---|---|
| what drives a turret seat's AI | **BF2:** the SEAT gets its own controller, a `UnitStationaryController`, made by the seat's own virtual whenever it has a pilot whose player index is negative - a bot that sat down, or the seat itself. It maps orders to three agents only: wait, combat, unboard (2). **BF1:** the same class, made only when a pilot is SET (4). |
| the self-piloted turret, BF2 | `PilotType = self` makes the seat its own pilot at construction; the armed building's spawn makes every such seat a `UnitStationaryController`; a pass every 15th AI step gives any self-piloted controller that has no order the order type 10, which the stationary controller maps to **UnitWaitAgent**. From there it is an ordinary AI gunner with nobody in it: it looks about, sees, fights (mode "stand and shoot" only, and only a target one of its weapons has in range), and cannot leave - the exit request needs a character (3) [O]. |
| the self-piloted turret, BF1 | **No controller, no AI at all.** BF1 makes a seat's controller only in SetPilot and has no order pass. BF1's two self-piloted classes (the Naboo fambaa's shield and its generator) work through two class flags the turret's own tick reads: `Controllable = 0` (the turret takes as its target whatever a ray of the weapon's LockOnRange along its fixed aim hits) and `AutoFire = 1` (the tick holds that weapon's trigger while the owner is alive) (4, 5) [O]. |
| "the MountedTurret acquisition" (B1 `FUN_00223850`) | It is that `Controllable = 0` ray. It runs only for a seat no human drives whose class says `Controllable = 0`: on the discs, the fambaa's two classes and nothing else. For every other turret an AI's target is assigned by its agent, not by the tick. `re-aim-assist.md` 3.10 and `re-vehicles.md` 9 read the gate as an entity field; it is the class's ODF flag (5) [O]. |
| what a self-piloted turret does, live | BF2 Hoth 1-flag CTF: 14 of them (13 tripod chainguns, one roof gun). Every one: a stationary controller, pilot = itself, no character, order type 10, UnitWaitAgent, from 1.1 s after the level starts. They hold soldiers as targets out to 100 m, fight inside weapon range + 10 m (80 + 10) and press the trigger only inside 77 m; 84-96 % of the time they look at a point (6) [L][O]. |
| the seat's trigger rule | **There is none of its own.** One fire controller serves every controllable kind and dispatches on the WEAPON: a cannon with `MaxPressedTime` > 0 is charged; else a weapon with the `TriggerSingle` bit is tapped; else a cannon-class weapon is fired in bursts; anything else is held while wanted. `TriggerSingle` defaults to 1 in the base weapon class and to **0 in every class built on the cannon constructor** (the labels `cannon`, `weapon`, `launcher`, `towcableweapon`, `grapplinghookweapon`) **and in repair**. So 103 of BF2's 127 distinct seat weapons (59 of 75 in BF1) get bursts, only what authors `TriggerSingle 1` or is of another class is tapped (the guided rockets; BF1's AT-AT chin cannon and hailfire missile), and one gun in each game is charged: Hoth's dish turret (7) [O]. |
| a turret's aim | BF2: the shared aim point (`re-walker-ai.md` 5) and, for a turret whose `MaxTurnSpeed` is over 0.5, the hover's stick law u·\|u\|, u = clamp(t·(4.5 − clamp(rate, 1, 3)), ±1); rates = the section's `MaxTurnSpeed` / `MaxPitchSpeed`. BF1: always the deadbeat law (2.2, 4) [O]. |

**For the port:** no conquest layer on either disc places a self-piloted turret (1), so "a turret that fires by itself"
is campaign / CTF / hunt content plus BF2's dispensed auto turret and BF1's fambaa - none of it in the port's conquest
today. What the alpha's bot gunners need from this note is 7 (charge the Hoth dish, tap what authors `TriggerSingle 1`)
and 2.3 (the look, the range band). Section 8 has the build.

## 1. What the discs ship [D] (tools in section 9; every number re-run by the lead)

- **`PilotType`** (hash 0x91ee6929): BF1 14 `PROP` chunks - `remote` 11, `self` 3; BF2 145 - `self` 112, `remote` 29,
  `none` 4. `vehicleself` is authored nowhere.
- **Self-piloted classes, BF1** (3): `imp_droid_probe` (a droid) and, in `NAB1_H.LVL` only, `nab_bldg_fambaa_generator`
  and `nab_bldg_fambaa_shield` (`commandarmedanimatedbuilding`; TURRET1: `PilotType self`, `Controllable 0`,
  `AutoFire 1`, `MaxTurnSpeed 0`, `MaxPitchSpeed 0`; the generator `YawLimits 150 210`, `WeaponAmmo 0`; the shield
  `WeaponAmmo 2`). `Controllable` (0xb371fc9a) and `AutoFire` (0x9d39f68a) have 2 `PROP` chunks each on the BF1 disc
  - those two classes - and **none on the BF2 disc**.
- **Self-piloted classes, BF2** (28 author it): the armed buildings `tur_bldg_chaingun_tripod`
  (weapon `tur_weap_chaingun_auto`: MaxRange 80, LockOnRange 48, ShotDelay 0.05), `tur_bldg_chaingun_roof`
  (`tur_weap_laser_auto`: 250 / 75), `tur_bldg_recoilless_fel_auto` / `_myg_auto` / `_nab_auto`, `uta1_bldg_turret`
  (map-local), the space frigates' and capital ships' turrets (12 classes); `com_bldg_inf_autoturret` (the
  `portableturret` a dispenser drops; `Lifetime 60.0`), three `defensegridturret` classes; a probe droid; four
  scripted gunships and a troop transport.
- **Where they are placed** (every world of every map LVL, each instance once): FEL1 5, HOT1 14, KAM1 11, MUS1 10,
  MYG1 4, NAB2 3, UTA1 2 - 49 on ground maps, 365 with the space maps. **None is in a world a conquest mission
  loads:** HOT1's are in `hoth_objective` (9 tripods and the roof gun, team 2; the world sits in the campaign, the
  1-flag and the hunt sub-levels) and `hoth_ctf` (4 tripods, team 1), KAM1's / MUS1's / FEL1's / NAB2's / UTA1's in
  the campaign worlds, MYG1's in `myg1_cp-assault`.
- **Which missions load a self-piloted class** (the scripts' `ReadDataFile` calls, `mission_census.py`): 30 of 128.
  Ground, Instant Action: **`hot1g_1flag` and `hot1g_hunt`** (both chainguns). Ground, campaign: `hot1g_c`, `kam1c_c`,
  `mus1c_c`, `fel1c_c`, `myg1c_c`, `nab2g_c`, `dea1g_c` (the transport), `uta1c_c` (map-local class, no explicit
  load). Loaded but placed in no world of that mode: `kam1c_con` / `kam1g_con` and `kam1c_1flag` / `kam1g_1flag` (the
  roof gun), `cor1g_con` (`all_bldg_defensegridturret`; no instance in COR1.LVL). The rest are space missions.
- **BF2 live** agrees: `hot1g_1flag` holds 14 self-piloted turrets, each within 0.1 m of a disc instance, each a
  different one: all 4 of `hoth_ctf` and all 10 of `hoth_objective` (6).

## 2. BF2: an AI in a turret seat

### 2.1 The seat makes its own controller [V][O]
- **C1.** The Controllable constructor `FUN_002f47a0(C, class, playerIndex)` stores the pilot C+0xbc = C itself when
  the class's PilotType (class+0x21c) is 1 (`self`) or 4 (`vehicleself`), else 0; C+0xc0 = C+0xc4 = the player index;
  C+0x140 = the PilotType (B2:283187-283202).
- **C2.** A MountedTurret's "make my controller" is `FUN_0026fcc8(T)` (B2:190815-190849), slot +0x84 of its
  Controllable vtable 0x620050 (thunk `FUN_00271378`, the data word at 0x6200d4): player index T+0xd0 (= C+0xc0) < 0
  and pilot T+0xcc (= C+0xbc) != 0 -> unless a network client (`DAT_00600548` / `DAT_00600549` / `DAT_00600560`), a
  new **UnitStationaryController** (`FUN_002b9078`, 0x620 bytes from pool 0x698118); a player index >= 0 -> a
  PlayerController (`FUN_002b0478`). Then `FUN_002f5130(C, controller, ...)` attaches it (C+0xb4, only when the seat
  has no controller yet) and `FUN_002aef80` files it in the AI manager's think list `DAT_005fe08c` (B2:283564-283610,
  233684-233730).
- **C3.** Three functions create a UnitStationaryController and nothing else does (`re_check.py callers b2
  FUN_002b9078`: 3; each reached through one thunk, one vtable word): `FUN_0026fcc8` (MountedTurret), `FUN_00271f88`
  (the same body for the Controllable at +0x10 of the next class in the file, vtable word 0x620394 [D: PassengerSlot])
  and `FUN_00233288` (an entity with its Controllable at +0x270, vtable word 0x61e294 [H: which class]).
- **C4.** SetPilot `FUN_002f5478(C, pilot)` (it returns at once when the pilot is unchanged) calls slot +0x84 after
  storing a non-null pilot (B2:283851) - so a bot that sits down gives the SEAT a stationary controller of its own -
  and, for PilotType 4 left with no pilot, makes the seat its own pilot again and calls it again (B2:283906-283911).
- **C5.** `UnitStationaryController`'s constructor (`FUN_002b9078`, B2:240556-240577; vtables 0x6220b8 / 0x622060 /
  0x622070; id `DAT_00698178`, name string 0x6220d8) runs the UnitController constructor and sets ai+0x581 = 1 when
  the seat's owner is an EntityBuilding (`DAT_00688e98`). ai+0x581 has two readers: the wait agent's look-point pick
  (2.3) and `FUN_002caaf0` (B2:253247, not opened [H]).
- **C6.** Its only overrides are the type functions, the destructor and the order map (vtable dump, section 9): the
  map `FUN_002b9118` (B2:240580-240615) sends order type 1 -> state 1, 2 -> 2, 5 -> 5 and **8, 9, 10, 0xb, 0xc -> 1**;
  every other type changes nothing. States (`re-soldier-ai.md` 1.4): 1 UnitWaitAgent, 2 UnitCombatAgent, 5
  UnitUnBoardAgent.

### 2.2 What the seat's AI writes each frame [V][O]
- **C7.** The MountedTurret's AI input is `FUN_0026fbd0(dt, T)` (B2:190784-190812), slot +0xcc of vtable 0x620050
  (thunk `FUN_002713f8`, data word 0x62011c):
*(decompiler excerpt omitted from the public copy)*
- **C8.** K+0x834 is `MaxTurnSpeed` (hash 0x5433f589, B2:191751-191755) and K+0x83c `MaxPitchSpeed` (0xb5534be0,
  B2:191464, 191505-191507); the class constructor `FUN_00270540` defaults them to 2.0 and 0.5 (B2:191385-191386).
- `FUN_002a50c0` / `FUN_002a45a0` are `re-walker-ai.md` 5 / 4 [O there]: the aim point is the target's lead point
  (order type 0), the order's point (type 1) or 30 m along the body's forward (type 2); a MountedTurret whose yaw
  rate argument is over 0.5 takes the hover's stick law, any other the deadbeat one. Every shipped self-piloted
  section authors `MaxTurnSpeed` 3.8 or 5.0, so k = 4.5 − 3 = 1.5.
- [L] The law on the PS2 (6; `stick_check.py`): predicted turn stick u·|u|, u = clamp(k · s, ±1), s = the sine of
  the yaw error between the seat's view direction and the target, against the stick at T+0x84, on the rows that aim
  at an entity (the model leaves out the lead and the aim error). On the 970 rows whose stick is over 0.1 the law
  with k = 1.5 is within 0.05 on 73.2 %, where the sign flipped gives 0.0 %, a linear stick 2.7 % and the deadbeat
  law 2.6 %. **What the data does not settle** (the verifier): 969 of those rows are ONE turret (the roof gun); a
  gain sweep gives k 1.4 49.0 %, 1.5 73.2 %, 1.6 87.6 %, 1.75 73.1 %, 2.0 25.3 %, so it separates the hover law's
  shape from the other laws, not 1.5 from 1.6; and every row has rate 5.0, so the rate term is untested here.

### 2.3 Waiting, looking, fighting (what differs for a seat) [V]
- The wait agent's update `FUN_002cd2e0` (B2:255069-255160; first transcribed in `re-soldier-ai.md` 7.3): the target
  holder runs (target = the threat list's best, the fire request); 1 think in 18 a new look point (`FUN_002cd1d0`,
  B2:255016-255066: the position + 1000 m along the stored base direction turned by a uniform yaw in ±75°, **±30°
  when ai+0x581**, B2:255037); on a seat whose owner reports speed 0: no live entity in the holder and the agent's
  clock (+0x70) over `DAT_00697e40` = 10 s old -> the leave request `FUN_002baa20(mv, 0)` **and return** (the aim
  order is not refreshed that think); else: a VISIBLE target -> aim order = the entity; only a remembered one ->
  its last point + 1.6 m; none -> the look point + 1.6 m (0x3fcccccd, B2:255126-255152).
- **C9.** A fight is accepted from a turret by the attack matrix's row for attacker kind 7, `FUN_002ac898`
  (B2:231764-231781): the weapon choice `FUN_002ab590` over the seat's weapons; the mode is set to 2 ("stand and
  shoot") only when a weapon is usable AND in range, else it stays 0 and the pair is refused. So a turret opens a
  fight only on a target inside one of its weapons' MinRange..MaxRange that the weapon's `AITarget*` mask allows
  (`re-soldier-ai.md` 4.3, 4.4 for the choice and the matrix).
- The fight itself, sight, the threat list and the trigger's aim test are the soldier's (`re-soldier-ai.md` 4.1,
  4.2, 4.9; `re-ai-difficulty.md` 5): seen through one ray; the attack ends when the target is over weapon range +
  10 m away; a shot needs the aim within 22.5° and the point inside weapon range.

## 3. BF2: the self-piloted turret [O]

- **C10.** The armed building's spawn `FUN_001e7e08` (B2:96540-96635; slot +0x1c of EntityBuildingArmed's vtable
  0x61a470 and of three classes derived from it) spawns each of its turrets (the array at +0x6b4, count +0x6d4) and
  then, unless a network client, calls slot +0x84 on EVERY seat of its seat ring (B2:96626-96633). With C1 and C2: a
  `PilotType self` section has pilot = itself and player index −1, so it gets a UnitStationaryController there; a
  `vehicle` section (pilot 0) gets none. Eight other entity functions hold the same ring loop (`grep "+ 0x84))("`:
  B2:100577, 104061, 105419, 114768, 131258, 147877, 150296, 176001; their classes not named [H]).
- **C11.** The order: `FUN_002af348` (B2:233833-233888), first call of `FUN_002af788` (B2:234002), which the AI tick
  `FUN_002af948` runs when its counter `DAT_005fe090` passes `DAT_005fe084` = 15 (B2:234086-234091; ELF word 15, no
  writer): for every UnitController of the think list whose Controllable has PilotType 1, or 4 with itself or nobody
  as the pilot, no character (C+0xb8 == 0), a current order of type 0 (ai+0xb0) and no order pending
  (`FUN_002b5d88`): **append an order of type 10** (`FUN_002b6258(order, 10, ...)`, the queue's slot +0x34). By C6
  that is state 1, UnitWaitAgent.
- **C12.** Without that order nothing would happen: a message reaches a controller only through its agent
  (`FUN_002b7e58`, B2:239716: `if (ai+0x420 != 0)`), the state machine starts at state 0 with nothing pending
  (`FUN_002adb90`, B2:232713-232724), and the two weapon functions that push an order to the controller of a
  self-piloted Controllable they hold skip a UnitStationaryController outright (`FUN_003338c0` B2:326960-326964 and
  `FUN_00335920` B2:328725-328729: IsA `DAT_00698178` -> no order; that the object is one the weapon has just
  spawned is [H]).
- **C13.** It cannot leave. The wait agent's 10 s rule still fires the leave request, and the mover's handler
  `FUN_002bbf00` clears the request bit and does nothing when the Controllable has no character (B2:242713-242718:
  `C+0xb8 == 0` -> skip the exit call). After those 10 s the wait agent returns before its aim code at every think
  until a threat is back in the holder (2.3) [D].
- [D] Its team is the owner's; it dies with the building (the building's death tears the seat's controller down) and
  gets a new controller when the building is rebuilt (C10 runs again): live, the roof gun went at 240 s and was back
  at 360 s with no agent until its next think (6).
- [D] `vehicleself` (4) is the same with a seat a soldier may also take (C4); no disc class uses it.
- **Not a rule of this code:** a range, cone or reaction of the turret's own. All of it is the unit AI's: the view
  distance by the target's size and the level's multiplier, the 77.5° cone about the seat's view direction (10 m all
  round), the think period by the level of detail (4 s unwatched, 1 s in a camera's view; 1.6 / 0.4 s in a fight).

## 4. BF1 [R = the reader's, re-opened by the lead where marked]

- **C14.** [V, lead] BF1's Controllable constructor `FUN_0023a648` also makes a `self` seat its own pilot (C+0xe4 = C
  when class+0x21c == 1; C+0xe8 = the player index; C+0xf0 = the PilotType; B1:153364-153375).
- **C15.** [V, lead] A seat's controller is made by `FUN_00225850(T)` (B1:138558-138597; slot +0x68 of vtable
  0x534310, thunk `FUN_002269c0`, data word 0x534378): no pilot -> return; the PILOT's player index < 0 -> a
  UnitStationaryController (`FUN_00265370`, 0x400 bytes), else a PlayerController. `FUN_00265370` has two callers in
  the ELF (`refs.py`: 0x225920, 0x227fa0 = this function and PassengerSlot's `FUN_00227ed0`); a second, default
  constructor `FUN_00265748` has no reference at all.
- **C16.** [R][D] That virtual is called from SetPilot `FUN_0023c488` only when the pilot CHANGES (the guard at
  B1:154789, the call at B1:154841) and from one soldier-side site (`FUN_0020c080`); the armed-building constructors
  (`FUN_001c4378`, `FUN_001c2f40`) allocate their turrets and never call it; an ELF scan for `lw rt,0x68(rb)` +
  `jalr rt` finds no seat-ring loop (the verifier's own scan: ten call sites through a Controllable vtable - SetPilot,
  that soldier-side one, eight on a unit's own Controllable - none walking a seat list; the BF1 function shaped like
  BF2's spawn, `FUN_001c46d0`, slot +0x18 of EntityBuildingArmed's vtable, has the first loop and then returns). So
  **a BF1 `PilotType self` turret never gets a controller**, and its AI input `FUN_00225780` (which only the AI
  update node runs) never runs.
- **C17.** [R][D] No order pass: BF1's manager tick `FUN_0025d988` (B1:179237-179329) has none, and the only loads of
  Controllable+0xf0 in the AI code (0x241000-0x266000) are two compares with zero. Not scanned: a pass keyed on the
  class field +0x21c [H].
- **C18.** [V, lead] BF1's stationary order map `FUN_00265410` (B1:185075-185102): order 1 or 3 -> state 1
  (UnitWaitAgent), 2 -> state 2 (UnitCombatAgent), 6 -> state 6 (UnitUnBoardAgent); nothing else.
- **C19.** [R] The AI input `FUN_00225780(dt, T)` (B1:138520-138553) is BF2's shape: `FUN_0023d418(C)` (the
  requests), `FUN_0023cd40(dt, C, aim)` (the fire controller), `FUN_00233200(dt, K+0x113c, K+0x1144, C, T+0x200, aim,
  &turn, &pitch, 0)`, sticks to T+0x78 / T+0x7c. K+0x113c takes `TurnRate` and `MaxTurnSpeed`, K+0x1144 `PitchRate`
  and `MaxPitchSpeed` (defaults 2.0 / 0.5). The stick law `FUN_00232b78` (B1:147710-148001) takes the rate branch
  only for an EntityFlyer or an EntityHover (B1:147762-147765), and a MountedTurret's entity getter returns the
  turret itself: **a BF1 turret always gets the deadbeat law**, on a building or on a hover alike.

## 5. `Controllable = 0` and `AutoFire`: the turret's own tick (both games) [O]

- **C20.** Both MountedTurret class setters parse the two keys: BF1 `FUN_00225f08` - `Controllable` -> K+0x1150
  (B1:139179-139183), `AutoFire` -> K + 4·slot + 0x1154, slot = the weapon being parsed (B1:139185-139189); class
  constructor `FUN_00225cd8`: Controllable 1 (B1:138897), AutoFire 0 [R]. BF2 `FUN_00270770` - `AutoFire` ->
  byte K + slot + 0x846 (B2:191458-191461), `Controllable` -> byte K+0x845 (B2:191466-191469); constructor
  `FUN_00270540`: K+0x845 = 1, K+0x846..0x849 zeroed (B2:191401, 191403).
- **C21.** BF1's tick `FUN_00223850` (thunk `FUN_00226860`, the vtable word 0x5342fc), in its branch for a seat
  whose player index T+0xf8 is negative (B1:137830): with a current weapon and **K+0x1150 == 0** and the weapon
  class's LockOnRange (wc+0xd4) > 0, a ray `FUN_00161d68(LockOnRange, origin T+0x110, direction T+0x120, ..., exclude
  the owner, ...)` and the entity it hits becomes the weapon's target, written every tick, 0 on a miss (weapon+0x128,
  its id at +0x12c, +0x130 = −1; B1:137839-137866). With K+0x1150 != 0 the tick only keeps the target somebody else
  assigned (dropping a stale handle) and aims at it, or 512 m along its aim (B1:137867-137927).
- **C22.** BF1's tick tail (B1:138079-138118): owner E+0xac bit 3 set, a current weapon, and K + 4·slot + 0x1154 != 0
  -> step that weapon's trigger byte at T + 0x58 + 2·slot as a press: 0 -> 2, negative -> 1, positive -> +1 up to
  100. [R] It is the "want" arm of the fire controller `FUN_0023cd40` statement for statement; with no controller
  nothing ever releases it, so it reads as a trigger held for good.
- **C23.** BF2 keeps both in its tick `FUN_0026cea0` (slot +0xc of vtable 0x620050): K+0x845 == 0 -> the ray of the
  weapon class's +0x104 (LockOnRange) (B2:189723-189729); the tail: owner +0x228 bit 3 set, a current weapon and
  K + slot + 0x846 != 0 -> the pressed encoding into the trigger word T + 0x48 + 4·slot (B2:189975-190030).
  **No BF2 class authors either key** (1), so both branches are dead with retail data [D].
- **So:** `re-aim-assist.md` 3.10's "runs when `this[0x3e] < 0` and `*(entity+0x1150) == 0`" is "no human in the seat
  and the CLASS says `Controllable = 0`"; with the default 1 the ray never runs. `re-vehicles.md` 9's "MountedTurret
  acquisition" is the same branch. Neither is how an AI gunner gets a target.
- [R] `Controllable = 0` also keeps the turret out of the list the AI's seat offers are drawn from (`DAT_00587774`,
  filled by the constructor only when K+0x1150 != 0, B1:136843-136845): no bot is offered the fambaa's seat.
- **Open for card FAMB** [H]: where the turret's own tick node gets linked for a seat nobody sits in (that
  `FUN_00223850` runs every frame for the fambaa); whether the shield and generator weapons fire on the trigger's
  level or on its press edge (AutoFire's byte never passes through the value 1); what those two weapon classes do.
  [D] from the disc: the GENERATOR's weapon `nab_weap_bldg_fambaa_generator` is a `repair`-class weapon with
  LockOnRange 50 (so the ray runs for it: it works on whatever stands within 50 m along its fixed aim, authored
  `YawLimits 150 210`); the SHIELD's `nab_weap_bldg_fambaa_shield` is a `shield`-class weapon with LockOnRange 0 (no
  ray; AutoFire alone keeps it on).
  **Closed 2026-10-16 by `re-fambaa-shield.md` 3-4:** the building's create links every turret's tick into the
  per-step update list; both weapons fire on the trigger's LEVEL (TriggerSingle 0: byte > 0); the shield weapon
  refills the shield per clip and the generator's repair pulse feeds it a clip every 0.1 s.

## 6. The PS2 itself: BF2 Hoth, 1-flag CTF [L]

Booted from the Instant Action planet list (Hoth, 1-FLAG CTF, the one era), the player left on the spawn map, software
renderer. `vtat_live.py` walks the AI manager's think list and keeps every controller whose Controllable has the
MountedTurret vtable; `live_an.py`, `live_decode.py`, `ram_turrets.py` print what follows.
- **Who they are.** 46 controllers at the start: 32 soldiers' and **14 turret seats**, each 0.02-0.07 m (x, z) from
  a different disc instance: `tur_bldg_chaingun_tripod15..18` of `hoth_ctf` (owner team 1) and `tripod1..6, 8, 9, 10`
  + `gun1` (`tur_bldg_chaingun_roof`) of `hoth_objective` (team 2).
- **C1-C6, C10 in RAM.** All 14, on every one of 86,605 rows (two battle captures): controller vtable 0x6220b8,
  PilotType 1, pilot == the Controllable itself, character 0, player index −1, ai+0x581 = 1. In the save state:
  K+0x834 / +0x83c = 5.0 / 5.0, K+0x845 = 1, AutoFire bytes 0 0 0 0.
- **C11 in RAM.** Every turret's first think is at 0.06 / 0.09 s of mission time with order type 0 and no agent; its
  second, at 1.09 / 1.12 s, has order type 10 and a UnitWaitAgent (vtable 0x623858); in between, at 0.42-0.52 s, the
  queued-order count ai+0x290 goes 0 -> 1 on every turret (the pass of the 16th update). 920 rows without an order,
  then none again except a rebuilt turret's first seconds.
- **What they did** (two captures, mission time 102-332 s and 333-397 s):

  | rows | order 10, wait agent, a look point | order 10, wait agent, aiming at an entity | order 2, combat agent, a target entity |
  |---|---|---|---|
  | 31,491 (10 Hz, 230 s) | 95.9 % | 1.4 % | 2.5 % |
  | 55,114 (62 Hz, 64 s) | 84.3 % | 5.2 % | 10.3 % |

- **Ranges** (second capture; the target's position against the seat's eye): in the combat agent the target was at a
  median 34 m (p10 17, p90 62, **max 89**); the trigger word's pressed bit was set only at **10-77 m** (median 32);
  the wait agent's entity targets stood at **88-100 m** (median 94; 450 of 2,869 rows under 90 m) and were never
  fired at. The tripod's weapon: MaxRange 80. So: seen and tracked to 100 m, fought to MaxRange + 10, fired at inside
  MaxRange.
- **Think periods** by turret: 4.0 s waiting unwatched and 1.6 s fighting (level 0), 1.0 s waiting in the spawn
  map camera's view (level 3) - the unit AI's table (`re-soldier-ai.md` 1.2).
- **They twitch when watched.** Three tripods in the camera's view re-thought every 0.03-0.1 s for long stretches and
  drew 344-471 look points in 64 s, their turn stick reaching 1.0, with no shot: the wait agent restarted over and
  over (each restart draws a look point) [D: a fight order that ends at once, re-thought at once at level 3]. The
  unwatched ones drew 1-5 look points in 230 s.
- **The trigger.** Pressed runs of the tripod chaingun (a cannon-class weapon, `TriggerSingle` bit 0 in RAM): 2.2 s,
  2.2 s, 2.1 s, 0.8 s, ... in the 62 Hz capture, 0.7-7.6 s in the 10 Hz one: bursts, not taps (7).
- **Death and return.** The roof gun (50 health) was among the 14 at 21 s, had no controller from before 102 s to
  181 s, fought, was gone again at 240 s and back at 360 s: 120 s, the armed building's default `RespawnTime`
  (`re-vehicles.md` 8). Each return: no agent until its next think (4 s and 1.3 s later), then order 10.
- The kept state `ps2/hot1g_1flag_battle.p2s` (mission time about 400 s) holds the 14 for any later RAM check
  (`ram_turrets.py`).

## 7. How a seat's gun is triggered (both games) [O]

- **C24.** BF2's fire controller `FUN_002bb1b0(dt, mv, aim)` (B2:242120-242362) is called by seven AI inputs, one per
  controllable kind (`re_check.py callers`: 7; the MountedTurret's at B2:190801). It takes the unit's current weapon
  from the Controllable (`FUN_002f59a8(C)`, B2:242147) and holds no test of the controllable's kind. The dispatch
  (B2:242167-242183), with W the weapon and wc its class:
  ```
  FUN_002bb0c8(W)                       -> FUN_002baac0   charge it
  else bit 50 of wc+0x1e0 (TriggerSingle) -> FUN_002baf70   tap (a sniper rifle: FUN_002baa38)
  else wc IsA WeaponCannonClass         -> FUN_002bac98   bursts
  else                                    the want stays as the agent set it: held
  ```
  then the line-of-fire veto `FUN_002bb900` and the trigger word of the weapon's channel. The sub-controllers are
  `re-ai-difficulty.md` 4.4 and `re-round-stalls.md` 2 C4 [O there].
- **C25.** `FUN_002bb0c8` (B2:242075-242118): a WeaponCannonClass weapon with class+0x240 > 0, or a
  WeaponGrenadeClass (`DAT_00699f40`) one with class+0x254 > 0, and not a WeaponLauncherClass (`DAT_00699f70`).
  Cannon class+0x240 is `MaxPressedTime` (hash 0x2819ffa0, B2:324833-324835; BF1 cannon class+0x1e0,
  B1:212000-212002).
- **C26.** `TriggerSingle` (0xed9d56db): the base weapon-class constructor SETS the bit (`FUN_00327e98`, B2:319538),
  the setter writes it (B2:321001-321026), the base copy constructor `FUN_003283d8` takes it from the parent class
  (B2:319677-319679), and two derived constructors CLEAR it: WeaponCannonClass's (`FUN_003304e0`, B2:324647; its id
  getter `FUN_00331620` returns `DAT_00699ed0`) and WeaponRepairClass's (`FUN_00347c20`, B2:340236; `DAT_00699fc8`).
  **Three more classes are built on the cannon constructor and never write the bit** (the verifier's correction): the
  labels `launcher` (`FUN_0033abf0`, B2:331993), `grapplinghookweapon` (`FUN_00337278`, B2:329327) and
  `towcableweapon` (`FUN_00349c80`, B2:341616) - so they start at 0 and are cannons by IsA. BF1 the same: base
  `FUN_00285a98` class+0xcc = 1 (B1:209496); cannon `FUN_002899c0` (vtable 0x53b1f8, `17WeaponCannonClass`) = 0
  (B1:211881); repair `FUN_00292e60` (vtable 0x53c830, `17WeaponRepairClass`) = 0 (B1:218694); `launcher`
  `FUN_00290680`, `towcableweapon` `FUN_00294e30` and `grapplinghookweapon` `FUN_0028e6b8` call the cannon constructor
  and never write +0xcc. [L] In the kept state the launcher and tow-cable label prototypes hold bit 50 = 0, the three
  infantry rocket launchers, whose chain authors `TriggerSingle 1`, hold 1 (the verifier's read).
- **C27.** The class labels `weapon` and `cannon` both build WeaponCannonClass: BF2's label table `FUN_001739d0`
  calls `FUN_003304e0` for the strings at 0x60f9d0 ("weapon") and 0x60fa00 ("cannon") (B2:16245-16255); BF1's
  `FUN_001727b8` calls `FUN_002899c0` for 0x527040 ("weapon") and 0x527048 ("cannon") (B1:15450-15454).
- **C28.** [R] BF1's fire controller `FUN_0023cd40` (B1:155147-155288; 7 callers, the MountedTurret's at B1:138545)
  has the same shape: a charge-type weapon -> `FUN_0023c840`; else class+0xcc == 0 -> bursts `FUN_0023c9c8` when the
  class is `DAT_00589418` (the cannon class's id: the verifier), else held; else (TriggerSingle) -> `FUN_0023cc20`,
  one press edge per 4 calls.
- [L] BF2 Hoth's tripod chaingun and roof gun in RAM: class vtable 0x628310 (WeaponCannonClass) for both - the roof
  gun's chain ends in the label `weapon` - with the TriggerSingle bit 0 and MaxPressedTime 0; and they fire in runs
  of seconds (6).
- **What that makes of the discs' seat weapons** [D] (`seatguns.py`: every weapon a vehicle body, a TURRET section or
  an armed building names, by its root label and effective keys; distinct weapons):

  | path | BF2 | BF1 | which |
  |---|---|---|---|
  | bursts (cannon by IsA, TriggerSingle 0, no MaxPressedTime) | 103 | 59 | nearly every vehicle and turret gun, the fighters' missiles and torpedoes, the snowspeeder's tow cable |
  | tap (TriggerSingle 1) | 19 | 12 | `remote` 10 / 0 (guided rockets), the remote droids' blaster and `destruct` 8 / 8, the droideka's `shield` 1 / 1; authored 1 in BF1: the AT-AT chin cannon, the hailfire missile, the Jedi fighter missile |
  | charge (cannon with MaxPressedTime) | 1 | 1 | `tur_weap_hoth_dishturret` (2.0 s) / `hoth_weap_bldg_dishturret_cannon` |
  | held | 1 | 3 | repair-class tools (in BF1 one of the two is the fambaa GENERATOR's weapon) and BF1's fambaa `shield` weapon |

  (BF2's other 3 name a weapon class that is on no LVL. The first version of this table took the launchers'
  default as 1 and filed 17 / 6 of the burst weapons under tap; the verifier caught it.)

## 8. The port, and the build

- **Today** (`core/src/ai/gunner.rs`, VTAI): a seated bot looks straight at the lead point of the target it SEES at
  the aimer's full rates (a deadbeat stick), and holds FIRE inside the weapon's range and its on-foot bursts; it
  never fires a seat weapon with `MaxPressedTime` (`gunner.rs:95`); with nothing seen the look returns to rest. Bots
  take gunner seats on hovers and walkers only; a `PilotType self` seat is merely not boardable
  (`game/src/vehicle_pads.rs` `seat_setup`); nothing drives it. No conquest layer places one (1), so nothing is
  silent in the port's battles today for THIS reason; BF2 Hoth's dish turret is silent in a bot's hands for the
  charge reason, once bots man turrets (card TURR).
- **Build, card VTAT (the gunner's trigger, alpha):** in `ai/gunner.rs`, the PS2's dispatch by weapon instead of the
  hold: a charge cannon is charged and released (re-ai-difficulty 4.4's rule: release at a charge drawn in
  [0.15 C, t C]); a seat weapon with the `TriggerSingle` bit is tapped by on-foot's `tap` (STTAP); a cannon keeps the
  bursts; the shot stays inside MaxRange and the fight inside MaxRange + 10 m. (The port's class defaults already
  agree with C26: `WeaponClass::default` false for the cannon-built classes, the grenade class true.) Checks: one
  scene test per path (a bot seated by hand at BF2 Hoth's dish turret fires at a target 60 m off; a bot at BF1's
  AT-AT chin cannon, `TriggerSingle 1`, fires more than once in 10 s), each mutation-checked; the sweep's `shots`
  census split by seat weapon before and after.
- **Build, new card VSELF (self-piloted turrets, post-alpha):** a seat whose PilotType is `self` gets a gunner with
  no soldier: the seat's team is its owner's, perception from the seat's eye, the wait / fight of section 3 (look
  point ±30° about the spawn facing, fight on a target inside a weapon's range band, never an exit), gone with the
  building and back with it. Content it unlocks: BF2 Hoth 1-flag CTF and hunt (14 placed, 14 alive on the PS2), the
  campaign's turrets (UTA1 2, MYG1 4, KAM1 11, MUS1 10, FEL1 5, NAB2 3), the dispensed auto turret. Check: the PS2's
  band on BF2 Hoth's `tripod16` at its disc pose - fires at a soldier at 60 m, tracks and does not fire at 95 m,
  ignores one at 110 m; a census line of auto-turret shots per map.
- **Notes for other cards:** FAMB - the fambaa's turret needs no AI: `Controllable = 0` + `AutoFire = 1` (5), and
  the open points listed there. TURR - BF1's turret stick is the deadbeat law, BF2's the hover law (2.2, C19); a
  building gunner's idle look is ±30° about its base direction, re-drawn 1 think in 18 (2.3).

## 9. Checks run, the verifier, what stayed [H]

- **Tools** (`engine/artifacts/lead_tmp/vtat/`): `pilot_census.py` / `inst_census.py` (classes and world instances
  with PilotType self; outputs `pilot_bf*.txt`, `inst_bf*.txt`), `mission_census.py` (which mission loads which
  class, over `scripts/gen_missions.py`'s bytecode reader), `seatguns.py` (`seatguns_bf*.txt`), `vt.py` (vtables side
  by side), `vtat_live.py` + `live_an.py` + `live_decode.py` + `first_order.py` + `stick_check.py` (the captures
  `ps2/ctf*`, `ps2/ctfb*`, `ps2/ctfc*`), `ram_turrets.py` (the state), `AGENT_BF1.md` (the BF1 reader).
- **re_check.py:** `callers b2 FUN_002b9078` 3, `FUN_002b8d10` 5, `FUN_002bb1b0` 7, `FUN_002af348` 1; `callers b1
  FUN_0023cd40` 7; `word b2 0x5fe084 1 --u32` = 15; `word b2 0x621f78 --str` / `0x6220d8 --str` = the two controller
  names; `word b2 0x60f9d0 / 0x60fa00 --str`, `word b1 0x527040 / 0x527048 --str` = "weapon" / "cannon"; `fn` on
  every cited line. `refs.py b2 0x26fcc8 0x26fbd0 0x271378 0x2713f8` (one `jal` each, one vtable word each);
  `refs.py b1 0x265370` (2 `jal`). `typeids.py` 0x688e98 EntityBuilding, 0x698178 UnitStationaryController, 0x699ed0
  WeaponCannonClass, 0x699fc8 WeaponRepairClass, 0x699f40 WeaponGrenadeClass, 0x699f70 WeaponLauncherClass.
  `hashloc.py` for AutoFire, Controllable, PilotType, MaxTurnSpeed, MaxPitchSpeed, TriggerSingle, MaxPressedTime;
  `propscan.py` for the PROP counts of 1.
- **Vtable dump** (`vt.py b2 0x621c90 20 0x621f58 20 0x6220b8 20`, `0x621c48 20 0x621f10 20 0x622070 20`): against
  the UnitController base, the stationary controller differs in +0x08 / +0x0c / +0x10 / +0x14 of the first and +0x14
  / +0x2c of the second (0x2b9220 -> `FUN_002b9118`); every other slot is the base's.
- **Verifier** (Opus, in the foreground, given `CLAIMS.md` only - 46 claims with their citations: A1-A23 BF2's
  code, B1-B13 BF1's, C1-C5 the disc censuses, D1-D5 the PS2 rows; it was told not to read this note or the
  reader's report; 161 tool uses, 33 minutes; `verify/VERDICTS.md`): **41 SUPPORTED, 5 PARTLY, 0 NOT SUPPORTED.**
  The five, all folded in above:
  - A22 / C5 (section 7): "no other constructor touches the TriggerSingle bit" missed that the launcher, tow-cable
    and grappling-hook classes are built on the cannon constructor (and the copy constructor copies the bit), so 17
    of BF2's and 6 of BF1's seat weapons the first table filed under tap are bursts. Re-run by the lead with the
    corrected `seatguns.py`: 103 / 1 / 19 (+3) / 1 and 59 / 1 / 12 / 3, the verifier's own table.
  - C3 (section 1): the instance census counted a world once per sub-level that embeds it - 49, not 70, on ground
    maps, and HOT1's 14 are exactly the 14 the PS2 holds (re-run with the corrected `inst_census.py`).
  - D4 (section 6): the wait agent's entity targets start at 87.9 m, not 90 (the tool printed p10).
  - D5 (2.2): the stick replay is one turret and cannot tell k = 1.5 from 1.6.

  It also: read four functions that have no decomp header by disassembly (0x2ade08, 0x331620, 0x348498, 0x1c14a0);
  ran its own virtual-call scans (`verify/vcall.py`) for both absence claims about the make-controller slot, with
  BF2's seat walk as the control; counted the loads and stores of ai+0x581 and K+0x845 / +0x846 over the whole text
  (`verify/offscan.py`); walked two turrets of the state by hand; cross-checked the mission census against its own
  LVL walk. Tool trap it found: `typeids.py` names 0x68c8f8 "MountedTurretClass", the registration pairs that id
  with "MountedTurret". Not checked by it: a re-capture (no emulator); the meaning of the flag quartet called the
  network-client case here; `scripts/gen_missions.py` was used, not audited.
- **Claim ids:** `CLAIMS.md` numbers its own way (A / B / C / D); this note's C1-C28 are the same statements in the
  order of the sections.
- **Lead only, after the verifier's pass:** the labels of the three cannon-built classes (`launcher`,
  `towcableweapon`, `grapplinghookweapon`: B2:16285, 16303, 16273; B1:15478, 15499, 15502), the two corrected census
  re-runs, and the wording of sections 0, 7 and 8 that follows from them.
- **Still [H]:** the class behind `FUN_00233288` (C3) and behind the other seat-ring loops (C10); `FUN_002caaf0`'s
  use of ai+0x581; the cause of the watched turrets' re-think storm (6); a BF1 order pass keyed on the class field
  (C17); everything listed under "Open for card FAMB" (5); that the two weapon functions of C12 act on an object
  they have just spawned. Not measured: a bot-manned turret's trigger on the PS2 (the SAIR captures hold seated
  gunners' rows; their weapon is a cannon, so bursts are expected), and the hover stick law's rate term on a turret.
