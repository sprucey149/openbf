# OPUS RE (2026-10-28) - [O] UNAUDITED — The remote droid: the `remote` weapon, the `droid` entity, its blaster and its destruct (card REMOTE)

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c` (SLUS-20898), B2 = `analysis/SWBFRNT2.ELF-decomp.c` (SLUS-21240); line
numbers are of those files. Tags: [V] read at the cited lines, [M] measured on the PS2 (PCSX2, live RAM), [D] derived,
[H] hypothesis, [O] load-bearing for the port. Anything not [V]/[M] is PROVISIONAL in code.
Disc values: the baked class chains (`engine/assets_baked/classes/{bf1,bf2}_*.ron`), read by name and by FNV-1a hash.
**No live PCSX2 capture was made for this note.** Nothing here is [M].

## 0. What the PS2 does, in one page [O]

| question | BF1 | BF2 | tag |
|---|---|---|---|
| who carries it | 8 soldier classes: the marksmen, the clone sharpshooter, the scout troopers, the sniper and assassin droids; channel 1, 1 clip | 13 classes counting variants: the officers and the Wookiee warriors; channel 1, 1 clip | V (disc census, `game/tests/suite/remote_droids.rs`) |
| the weapon | class `remote`, a `dispenser` subclass: the press throws the droid entity named by `OrdnanceName` like a mine; one droid per weapon; no second throw while it lives | same (`OffhandWeapon 1` on `com_weap_inf_remotedroid`) | V |
| launch | dispenser throw: aim x (MinStrength + (MaxStrength - MinStrength) x charge) x `Velocity` + the owner's velocity, at `InitialSalvoDelay` 0.3 s | same | V |
| control | 0xc43b77dd = 1: the soldier's controller moves to the droid at launch; the soldier stands | same | V |
| retaking control | a fresh press of the remote weapon's trigger while its droid lives | same | V |
| giving control back | the exit button or a forward weapon switch on either channel, or the droid's end; the droid stays where it is until it runs down. The AI presses none of them: a bot flies its droid until it ends (§6) | same | V |
| flight | hover on a lift spring over the ground ray (`SetAltitude` 1.75 / imp 2.0), stick drives toward `MaxSpeed` 9 / `MaxStrafeSpeed` 7 under an `Acceleraton` cap of 70 (BF1 only reads the misspelled key) | `SetAltitude` 2.0, `Acceleration` 25 | V |
| how long it lasts | `MaxHealth` 40, `AddHealth` -1/s: 40 s | `MaxHealth` 100, `AddHealth` -1/s, `EnergyAutoRestore` -1.7/s: energy out first, about 58.8 s | V, H (energy max 100); the port: 90 s in both, §7 |
| what it fires | channel 0 `*_remotedroid_blaster`: single-shot bolt every 0.2 s, 60 m/s, 10 damage, no vehicle/building damage | 0.15 s, 160 m/s, 60 damage, vehicle/building x 0.1 | V |
| the destruct | channel 1 `*_remotedroid_destruct`: one fresh press starts a 3.0 s (`MaxPressedTime`) countdown that cannot be cancelled -> an orbital salvo: 4 rounds 0.75 s apart, from 600 m at 70°, spread 2.5 m, each a 1000-damage bolt that explodes for 500 over 10.5 m | the same press and countdown -> the incinerator: a 12 m, 10000-damage burst at the droid (person/droid/animal only), plus `com_sfx_weap_remotedroid_exp` | V |
| its health | shot and blasted like a droid (`DroidScale`); destroyed -> its `ExplosionName` (BF1 50 damage over 3/5 m; BF2 a 0-damage look) | same | V |
| bots | never launch one: BF1's weapon choice has no remote branch and gives a weapon with no ordnance 0 (§6) | the weapon choice takes the remote at a fixed 1000 against any target, inside its default 0-60 m band, by the choice's usual order and "plenty" rule; the soldier's controller then flies the droid as its own body: its scan, target, move order and aim order, through the shared steering (§6) | V |

## 1. Class labels [V]

| label | BF1 | BF2 |
|---|---|---|
| weapon `remote` | `FUN_00292350` (B1:217943), derives from the dispenser `FUN_0028de58` (B1:214816) | `FUN_003465f8` (B2:327129), derives from the dispenser `FUN_00336858` (B2:316613) |
| weapon `destruct` | `FUN_0028bbb8` (B1:213118) | `FUN_00332ff0` (B2:314102) |
| entity `droid` | `FUN_001cb208` (B1:74919), size 0x25c0, base the common controllable `FUN_001e30c8` (B1:91993) | `FUN_001f0cf8` (B2:102731) |
| droid class setter | `FUN_001cb768` (B1:75160) | `FUN_001f12c0` (B2:102994) |

## 2. The remote weapon [V]

- 0xc43b77dd (disc 1) -> class+0x410 (B2 +0x4c0): take control on launch.
- 0xed422f60 (default 1) -> +0x414 (B2 +0x4c1): enables the "kill own droid" branch of the update.
- Fire `FUN_002921e0` (B1:217850): the dispenser fire `FUN_0028d958` (B1:214512); if the thrown item is a controllable it
  keeps its handle at weapon+0x150 (one droid per weapon) and, with +0x410, hands the soldier's controller (soldier+0xe4)
  to it through vtable +0x58.
- `FUN_00292100` (B1:217802) / `FUN_00292158` (B1:217826): no new fire while that droid lives.
- Update `FUN_00291ef8` (B1:217682): droid alive and a fresh trigger press (+0x70 == 1) -> control back to the droid.
  A press of a second byte (+0x74, unidentified; [H] reload) with +0x414 set kills the droid through
  `FUN_001bbc98(droid+8, …)` (B1:63906).
- The weapon's destructor kills its droid.
- Disc: `RoundsPerClip` 1, `ShotDelay` 1.4, `InitialSalvoDelay` 0.3, `AnimationBank` grenade, the thermal-detonator
  hand model, `FireSound com_weap_inf_remotedroid_fire`, `OrdnanceName <side>_weap_inf_remotedroid_ord`. The slot is 1
  clip.

## 3. The droid class [V]

Constructor defaults (identical in both games unless noted): `SetAltitude` 0, `GravityScale` 1, `LiftSpring` 5,
`LiftDamp` 2, `Acceleraton` 70, `MaxSpeed` 9, `MaxStrafeSpeed` 7, `MaxTurnSpeed` 2, `MaxPitchSpeed` 0.5, `PitchLimits`
±1 rad, `LevelSpring` 10, `LevelDamp` 3, CollisionThreshold 7.5 / 2.5.

- The hover-style keys on the disc (`ForwardSpeed`, `ReverseSpeed`, `strafeSpeed`, `turnRate`, `pitchRate`, `traction`,
  `deceleration`) are NOT read by the droid class; the defaults above govern speed and turning.
- BF1 matches only "Acceleraton" (misspelled); the disc authors "acceleration" 1.0, so BF1 keeps 70. BF2 also matches
  "Acceleration" (disc 25).
- `GravityScale` is authored "1.0//2"; the number parse stops at the comment, so 1.0.
- Authored: `SetAltitude` BF1 imp 2.0 / others 1.75, BF2 2.0; `LiftSpring` 10; `LiftDamp` 3; `LevelSpring` 5;
  `LevelDamp` 3; `PitchLimits` -50..50°; `EyePointOffset` (0, 0.5, 0); `TrackOffset` (0, -0.25, 1.5) BF1 / (0, -0.25,
  1.8) BF2; `TiltValue` 5.
- Health: `MaxHealth` BF1 40 / BF2 100, `AddHealth` -1 (per second, a drain). 0x465909fb = 30 and 0xeb460a76 = 300 sit
  in the health block, meaning unknown.
- BF2 `EnergyAutoRestore` -1.7: energy drains 1.7/s; at energy <= 0 `FUN_001ee610(1.0)` (B2:100734) is called. [H] that
  kills it and energy starts at 100, so 58.8 s.
- BF2 0x83e12771 (0, 0, 18) and 0xe11ce401 10° are an alternate camera offset/pitch while droid state +0x494 is 1..3
  ([H] the destruct charge). 0xf9765272 is the effect `com_sfx_weap_remotedroidcharge2`.
- `ExplosionName`: BF1 `*_remotedroid_exp2` (50 damage, radii 3 / 5, DroidScale 1); BF2 `*_remotedroid_exp1` (0 damage).
- WEAPONSECTION 1 `*_remotedroid_blaster` (channel 0, ammo 0 = infinite); WEAPONSECTION 2 `*_remotedroid_destruct`
  (channel 1, ammo 1).
- `AmbientSound remotedroid_engine_lp`, `Ambient2Sound remotedroid_radio`; geometry `com_weap_probedroid`; health icon
  0x0db7468e `HUD_com_probedroid_health_icon`; `AISizeType` BF1 SMALL / BF2 SOLDIER; `PilotType remote`
  (controllable class+0x21c = 3).

## 4. The flight step [V] — BF1 `FUN_001c9fd8` (B1:73969)

Arguments dt, forward stick, strafe stick, turn stick. With a = `SetAltitude` > 0:

1. Ray down 10a: ground distance h, normal n (no hit: h = 10a, k = 0).
2. k = clamp(1 - (h - a) / (9a), 0, 1).
3. vel.y += ((GravityScale - 1) k + 1) (-9.8) dt — gravity FIRST.
4. f = k 9.8 + LiftSpring (a - h) - LiftDamp (vel · n), using the velocity after step 3; f > 0 -> vel += n f dt.
5. Frame: forward / right from the yaw, perpendicular to the frame normal (n.x k, n.y + 0.1, n.z k) normalised.
6. c = Acceleraton k max(n.y, 0) dt; vel += fwd clamp(fwdIn MaxSpeed - vel · fwd, ±c); the same along right with
   MaxStrafeSpeed.
7. Yaw rate = -MaxTurnSpeed x turn (the AI: MaxPitchSpeed + (MaxTurnSpeed - MaxPitchSpeed) x factor).
8. The body up vector levels toward n through LevelSpring / LevelDamp; pos += vel dt. World collision is the common
   controllable's.
9. Speed² > 25 casts a second ray along the velocity that adjusts n.

[D] Because gravity is applied before the damping term reads the velocity, the steady state is not a: with v = -9.8 dt
after step 3 and n = Y, f = 9.8 k needs LiftSpring (a - h) - LiftDamp v to cancel the gravity deficit, which settles at
h - a = LiftDamp 9.8 dt / (LiftSpring + 9.8 / (9a)): 0.092 m at 30 Hz for BF1 (1.84 m for an authored 1.75).

## 5. The destruct [V]

- `MaxPressedTime` 3.0, `RoundsPerClip` 1, `ChargeSound` BF1 `timer_countdown` / BF2 `com_weap_remote_charge_fire`,
  0x4c5aa5aa = 0.3 (the charge sound's pitch step).
- BF1 instance vtable 0x53b5d0 (`python scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 53b5d0 48`): +0x74 fire entry
  `FUN_0028b898` (B1:212999) spawns a salvo object `FUN_00271f90` (B1:194436) when the class has an ordnance; +0x7c
  `FUN_0028b9d0` (B1:213069) explodes the class's ExplosionClass (0xca0c1378 -> class+0x204, if any) at the droid and
  kills the droid; +0x80 charge start `FUN_0028b7a8` (B1:212962: timer +0xd0 = 0, limit +0xd4 = class+0x200 =
  `MaxPressedTime`, 0x2819ffa0 in the setter `FUN_0028bcd8`); +0x84 accumulation `FUN_0028b868` (B1:212988).
- The predicates (not in the decompile; `python scripts/tools/eedis2.py bf1 0x0028bb08 0x0028bbb8`): charge +0xa4 at
  0x28bb08 — idle: the trigger byte == 1 (a fresh press); charging: timer < limit, the trigger NOT read; fire +0xa8 at
  0x28bb50 — charging and limit <= timer. Through the base state machine `FUN_00284318` (B1:208134; state 4 = charge:
  fire predicate -> fire, else charge predicate -> accumulate, else end) one press runs the countdown to the end and
  fires the destruct at `MaxPressedTime`, held or not; it cannot be cancelled. [V]
- BF2 the same: instance vtable 0x628690, charge start +0xb4 `FUN_00332740` (B2:313782: timer +0xbc = 0, limit +0xc0 =
  class+0x260), accumulation +0xb8 `FUN_003327f8` (B2:313808); predicates `python scripts/tools/eedis2.py bf2 0x00332f38
  0x00332ff0`: charge 0x332f38 — idle: trigger bits 0-1 == 3 (pressed), charging: timer < limit; fire 0x332f88 —
  state 3 (BF2's charge) and limit <= timer. [V]
- Salvo setter `FUN_002725a0` (B1:194686): [0] 0x3186fbfe yaw (deg -> rad, disc 0); [1] 0x8339fb35 pitch (disc 70°,
  default π/2); [2] 0xecdb9d1d distance (disc 600, default 100); [3] `SalvoCount` 4; [4] `SalvoDelay` 0.75; [5]
  `InitialSalvoDelay` 0; [6] 0xebd81a5e spread 2.5; [7] the ordnance (0x6fd92be8 in BF1, `OrdnanceName` in BF2).
- Salvo update `FUN_00272078` (B1:194482): M = rotX(-pitch) rotY(yaw); each round starts at target + M (spread (r1 -
  r2), spread (r3 - r4), distance) and flies along -M.z, r1..r4 from the game LCG.
- BF1 round `*_orbital_attack_salvo_ord`: bolt 300 m/s, life 3 s, 1000 damage, explodes as
  `*_orbital_attack_salvo_exp` (500 damage, radii 10.5 / 10.5, push 10).
- BF2 `com_weap_droid_incinerator_ord`: an `emitterordnance`, radius 12, ConeAngle 120, MaxTargets 10, 10000 damage,
  Person / Droid / Animal x 1, Vehicle / Building x 0; its explosion `com_weap_droid_incinerator_exp` is a 0-damage look;
  0x76f3beea is the sound `com_sfx_weap_remotedroid_exp`.

## 6. The AI

- **Launch, BF2** [V]. The suitability B2 `FUN_002aaba0` (B2:218293) asks a weapon class with no ordnance (+0x88 = 0;
  a dispenser keeps its item at +0x240, `re-portable-turret.md` 6.1) whether it IsA `DAT_00699fb8` =
  WeaponRemoteClass (its name getter 0x346920 builds "WeaponRemoteClass", 0x629c50): yes, and its item +0x240 is no
  `DAT_00689370` = EntityFlyerClass (getter 0x212820 -> 0x61bef8) -> score 1000, accepted (B2:218316-218334). This
  returns before the empty test `FUN_00327820`, the EntityTrap refusal, the AI mask and the too-weak rule, so the
  remote is suited to every target: a soldier, a vehicle, a droid. The choice `FUN_002ab590` (B2:218646) then treats
  it as any weapon: it must have ammunition (`FUN_003037e8` > 0), it is in range when MinRange (+0x110) < d < MaxRange
  (`FUN_002a9c30`, +0x118), and a later slot replaces the best only by the "plenty" (issued rounds > 2, `FUN_002a9ef0`)
  and score rules (`re-grenades.md` 7.1). The remote authors no AI range on the disc (census of the BF1 and BF2 side
  classes: `com_weap_inf_remotedroid` writes RoundsPerClip 1, ReloadTime 0, ShotDelay 1.4, OffhandWeapon 1), so the
  weapon class constructor's MinRange 0, OptimalRange 30, MaxRange 60 stand (B2 `FUN_00327e98` B2:307233-307234,
  a local[0x45] = 30.0, [0x46] = 60.0; [0x44] = 0 at B2:307248). With 1 round it is never "plenty": a plenty weapon
  in range keeps it out, and it is taken past the band of the weapons before it. The pick is applied by
  `FUN_002bbed0` and fired by the fire controller `FUN_002bb1b0` like any weapon in hand.
- The other BF2 selectors of an AI weapon (the callers of `FUN_002bbed0`: B2 `FUN_002b8160` WeaponInvisibility,
  `FUN_002b8258` OrdnanceEmitterClass, `FUN_002b8400` a dispenser of EntityBuildingArmedClass, `FUN_002cf458`
  WeaponMelee, `FUN_002d4bf0` a WeaponDispenser of EntityMineClass, `FUN_002d6d98` WeaponRepairClass /
  PowerupItemClass) test no remote. [V]
- **Launch, BF1: never** [V]. BF1's suitability `FUN_002380a8` (B1:151347) has one fixed 1000 for a class with no
  ordnance (+0x5c = 0), and its class id `DAT_005894c8` is WeaponMeleeClass (getter 0x291d80, name getter 0x291d90 ->
  "WeaponMeleeClass" 0x53c450), not WeaponRemoteClass (`DAT_005894d8`, getter 0x2926e0 -> 0x53c630; no other read of
  that word in B1). Any other weapon with no ordnance ends at *(decompiler detail omitted)* and is refused. BF1's other
  selectors (the callers of `FUN_0023d3f0`: `FUN_0023bc48` WeaponTowCable, `FUN_002456e0` the droideka's slot 1,
  `FUN_00254d60` a WeaponDispenser of EntityMineClass, `FUN_002566d8` WeaponRepairClass / PowerupItemClass) test no
  remote. So no BF1 bot launches a droid, and BF1's `UnitDroidController` serves droids piloted by AI in other ways.
- **A bot's droid is its own body** [V for the structure; the behaviour states that set the orders were not read].
  At launch the soldier's controller moves to the droid (droid +0x324). The droid's AI input B2 `FUN_001f02e0`
  (B2:102132; thunk `FUN_001f2038`, droid controllable vtable word 0x61b3e4) is the hover AI's H1 and H5 with none
  of its extra rules (`re-hover-ai.md` 2): it spends the controller's seat requests (`FUN_002bbf00`), copies the aim
  order mv+0x70, runs the fire controller `FUN_002bb1b0` on it, and steers by the shared steering `FUN_002a54d8` with
  the droid class's MaxSpeed +0x5c4, MaxStrafeSpeed +0x5c8, MaxTurnSpeed +0x5cc, MaxPitchSpeed +0x5d0 (setter
  `FUN_001f12c0`: hashes 0x86df0364, 0x3437a33d, 0x5433f589; ctor defaults 9 / - / 2 / 0.5), the controllable
  droid+0x270, the move order mv+0x20 and that aim; the sticks go to droid +0x2dc..+0x2e8. So the droid goes where
  the bot's own move order points and looks and fires where its aim order points: the scan, the target, the weapon
  choice (now among the droid's blaster and destruct) and the behaviour states run as for any body the controller
  drives.
- The shared steering for a droid (`re-walker-ai.md` 3, 4) [V]: the arrival radius is the soldier's 0.5 m (kind 1,
  `FUN_002a63e0`); a droid strafes, so throttle and strafe are the B8 pair toward the goal and the turn always goes to
  the AIM point (`FUN_002a50c0`: the target's lead, else 30 m toward the goal or ahead) by the deadbeat yaw law
  turn = clamp(-(n.X) / (MaxTurnSpeed dt), +-1) and the walker pitch law at MaxPitchSpeed.
- The droid's update applies the sticks at an AI pilot's rates [V]: yaw at MaxTurnSpeed and pitch at MaxPitchSpeed
  when the droid has no local player (droid +0x330 < 0; B2:101233-101240 for the pitch, B2:101991-102001 for the yaw,
  where a player's yaw rate is MaxPitchSpeed + (MaxTurnSpeed - MaxPitchSpeed) x its look factor +0x360). Its
  throttle and strafe are zeroed while droid +0x494 == 1 or a weapon in hand charges (B2:101215-101228).
- Targeting a droid [V]: the soldier's scan (B2 `FUN_002e3aa0`, B2:257960) takes every enemy-team entity of the
  spatial query, the droid among them, and asks the attack matrix (B2 `FUN_002ac8f8`, B2:219521) of each. The matrix
  switches on the target category of `FUN_002a6138` (B2:214910; B1 `FUN_00234c00`, B1:149003), where 1 is
  `EntityDroid` in both games (the class name at `DAT_00689298`, read in the ELF). Soldier against category 1 is
  `FUN_002abe18` (B2:219023), the same body as soldier against soldier `FUN_002abd38` (B2:218993): both pass the
  attacker's weapon set to `FUN_002abc38`, the weapon choice. BF1's is `FUN_00238f38` (B1:151981) -> `FUN_00238d10`.
  So a droid is an ordinary candidate: it is taken when a weapon the soldier holds is suited to it, by the mask
  (BF2 only, the `AITargetDroid` bit for `HealthType droid`) and the too-weak test (`re-soldier-ai.md` 22).
- Its view distance is the soldier's shape `(20 x radius + 30) x mult x vis` (B2 `FUN_002e3910`, B2:257901) with
  the droid's bound radius. [V] for the shape; the radius is the port's collision sphere, not the model's bound. [H]
- The aim error at a droid is the BF2 radius at size 0.17 (`re-soldier-ai.md` 21.3, measured), BF1's at size 1. [V]
- Control is handed over through the soldier, not the controller [V]. The remote's fire and its retake (B2 `FUN_00346480`
  B2:327045, `FUN_00346100` B2:326852) call slot +0x74 of the owner's piloting controllable: the soldier's controllable
  vtable 0x61e998 +0x74 -> `FUN_00255a28` -> `FUN_0024c560` (B2:165402), which keeps the droid at soldier +0x1084..+0x108c
  and makes the soldier its pilot (droid vt+0x88). Slot +0x78 -> `FUN_00255a48` -> `FUN_0024c778` (B2:165517) undoes it:
  control back to the soldier. BF1 uses slots +0x58 / +0x5c of the vtable at +0x294 (B1 `FUN_00291ef8` B1:217678).
- Who calls the hand-back [V]: only the droid itself, in two places, both gated on the class's pilot type 3 (`PilotType
  remote`):
  1. Its update, B2 `FUN_001ee668` (B2:101050-101103), B1 `FUN_001c9398` (B1:73480-73540), when no destruct state is set
     (B2 droid +0x494 == 0; B1 has no such test): the exit button pressed (B2 controllable +0x50, read `& 3 == 3`; B1 the
     action byte +0x2f2 == 1); or damageable field +0x18 above 0 (B2 droid +0x188, B1 +0x20: a timer the damageable's
     tick counts down to 0, B1 `FUN_001bbbe0` B1:63869; what sets it was not found [H]); or a weapon switch forward on
     either channel (B2 +0x2ec / +0x2f0, B1 +0x310 / +0x314 = soldier[0xc4+ch], the switch input whose sign is the
     direction, `re-grenades.md` 2.4).
  2. The controllable's teardown, B2 `FUN_002f4e00` (B2:271153-271290), B1 `FUN_0023ac68` (B1:153392-153512): pilot
     type 3 -> the hand-back. So the droid's end (shot down, run down, destruct, out of the world) gives control back.
- The AI never presses any of these [V]. In BF2 the only controller writer of the exit button is the PlayerController
  update `FUN_002b0548` (B2:223558-223741); the switch inputs are written by no AI code, only zeroed by
  `FUN_002f4a50` (B2:270966). The AI's own "get out" request (`FUN_002baa20` sets bit 3 of its order block +0xa8,
  B2:229546) is spent in `FUN_002bbf00` (B2:230407-230490) on the pilot's slot +0x70, the seat exit `FUN_0024ba68`
  (B2:164849), which needs a vehicle seat (soldier +0x328 -> +0x14c) and does nothing for a remote. BF1 hands an AI
  pilot's droid its own `UnitDroidController` (built by the droid's slot +0x68 `FUN_001cabd8` B1:74527 when the pilot
  is no player; ctor `FUN_002644e8` B1:184047), whose orders map to UnitCombatAgent (2), UnitTrooperAgent (3) and
  DroidRepairAgent (7) (`FUN_00264538` / `FUN_002645d8` B1:184070-184140); no BF1 AI code writes the action byte or
  the switch inputs. **So a bot flies its droid until the droid ends.**

## 7. Port decisions

| decision | tag |
|---|---|
| ACTION returns control; the droid stays until its time runs out; a fresh press of the remote's trigger retakes it | H |
| Lifetime: every droid, BF1 and BF2, all maps, players and bots, lasts exactly 90 s from launch (2700 ticks, the launch tick included; `remote::LIFETIME`). The timer is separate from health: health starts at the class's `MaxHealth` (BF1 40, BF2 100) and only damage lowers it; `AddHealth` and BF2's `EnergyAutoRestore` are read but not applied (the PS2: BF1 40 s, BF2 about 58.8 s, §0) | OURS (maintainer, 2026-10-28) |
| Timeout (`DroidEnd::Expired`): the droid vanishes quietly, with no explosion, no destruct and no damage; control returns to the soldier. A destruct countdown still running at 90 s is dropped. The spent clip follows the same rule as after any other ending (a restock relaunches) | OURS (maintainer, 2026-10-28) |
| The HUD's stamina bar shows the droid's time left (90 s full) while flying, in both games; the health bar shows its health | OURS |
| One fresh press of the droid's secondary trigger starts the 3 s countdown; it runs to the destruct whatever the trigger, the control hand-back or the owner does next (until the droid dies) | V (§5) |
| The BF2 incinerator is a direct radius-12 burst with the ordnance's scales, under the normal blast friendly-fire rule; MaxTargets and the 120° cone are not modelled | H |
| The owner's death ends the droid (`DroidEnd::OwnerDied`): it vanishes with NO explosion (was: its explosion went off, H) | OURS (maintainer, 2026-10-28) |
| The owner boarding a vehicle only returns control | H |
| Explosions: only shot down (`Destroyed`) and the self-destruct (`Destruct`) set off the droid's `ExplosionName` and its effect and sound; the self-destruct is unchanged (3 s countdown, BF1 salvo / BF2 burst) | OURS (maintainer, 2026-10-28) |
| Bot use: card DROIDAI's rules, §7.1 (was: launch whenever an enemy target was within 40 m; no destruct) | OURS (maintainer, 2026-10-28) |
| Control goes back on a fresh ACTION, `SWITCH_WEAPON` or `SWITCH_SECONDARY` press (the exit button and the forward switch on either channel), or when the droid ends. The PS2's AI never hands back (§6); the port's bots hand back by card DROIDAI's rules (§7.1). Not modelled: the damageable +0x18 timer (its setter is unknown) and BF2's no-hand-back while the destruct state is set (§3's +0x494 reading is [H]) | V |
| Bots target enemy droids (issue #58) as the PS2's scan does (§6): a droid is a contact and a target like a soldier, admitted by the weapon suitability test (BF2's mask, both games' too-weak test), in the view cone, the view distance and line of sight; a foot soldier aims at it with the size-0.17 BF2 radius (BF1 size 1) and the owner's error, a seated gunner by the same rule. The hash of a unit contact or target is unchanged | V |
| The map edge (issue #59): the PS2 tests the controllables a member drives against the `bnd_` boundary and kills what is still outside when the 10 s / 6.5 s countdown ends (`re-boundary.md` T1, T2). A flown droid takes that countdown over the `bnd_` shapes (`core::boundary`, `game::map_boundary`); it is in the slot only while flown (T2b), so one handed back is left alone. The player gets "Leaving Battlefield" pulsing at the screen's centre and his team's leaving line once an excursion (T4, T5, T1b). The soldiers and vehicles are card BNDY | V |
| A droid stops at the end of the terrain sheet, on every map (the only limit where no `bnd_` shape exists; the PS2 flies on) | OURS |
| With no ground under it (`NO_GROUND`, on a map with no terrain sheet past its floors) the stick drives nothing and the droid falls. The PS2's falls to y -500 and dies (B1:73674, B2:101534). The port's is destroyed after `remote::ADRIFT_LIMIT` = 1.5 s, about 11 m, so the player does not watch a 10 s fall. It used to take -300 for ground and fly on under the map | V for the drive, OURS for the floor |
| A player's pitch turns at `MaxTurnSpeed`; an AI pilot's at `MaxPitchSpeed` (§6, B2:101233-101240) | OURS for the player, V for the AI |
| Collision sphere 0.35 m | OURS, provisional |
| The frame normal's (n.x k, n.y + 0.1, n.z k) shaping and the velocity ray (step 9) are omitted; the frame uses n | OURS |
| Engine and radio loops are not played | gap |

### 7.1 The bots' droid (card DROIDAI, 2026-10-28) [ours]

The maintainer's playtest note: a bot launched its droid every time one was ready and an enemy was near, then stood
in the open, a free kill, flying a droid with a worse gun than its own. The rule now: a droid carrier sends its droid
into a fight it is not in, from where no enemy sees it, some of the time; flies a while; then goes in itself. The PS2's
own bot use of the remote is now read (§6, §0's bots row) and differs: BF1 bots never launch, BF2's weapon choice takes
the remote at a fixed 1000, and the bot flies the droid as its own body, never hands it back and never destructs. The
maintainer's rules stand in the port by choice (2026-10-29). Constants in `core/src/remote.rs` (`BOT_*`), the launch in
`core/src/ai/abilities.rs` (`launch_ok`), the flight in `core/src/sim/remotes.rs` (`bot_droid_pad`).

**When it launches** (all must hold; the cheap tests first):

1. the remote is ready (a clip, not reloading) and its last droid is gone;
2. 30 s have passed since its last flight ended (`BOT_REST`; `AiState::droid_rest`);
3. this 2 s window's draw passes, a 25 % chance (`BOT_ROLL`, `BOT_CHANCE`; the per-unit hash draw, sim state only);
4. the body was not hit in the last 3 s (`BOT_HIT_QUIET`);
5. a fight is near: an enemy the bot knows of, by its own eyes or ears or a friend's telling (its contacts), is within
   60 m (`BOT_FIGHT`);
6. no enemy is within 20 m (`BOT_NEAR`), seen or not: then it fights with its own gun;
7. no enemy within 60 m has a line of sight to its body (from the enemy's muzzle to the body's centre).

A bot reaches cover the usual way: the Tactician's hide phase, a guard post, or a wall between it and the fight. No
separate walk to a hiding spot was added (a judgment call: the Tactician's position and cover scoring already put
fighters in hidden spots, and a second mover for the droid would compete with them).

**While it flies** (each tick, before the droid's own steering):

- the flight ends when the body is hit, when an enemy within 15 m (`BOT_GUARD`) has a line of sight to the body, after
  30 s in the air (`BOT_FLIGHT`), or after 10 s with no enemy within 40 m of the droid (`BOT_RECALL`);
- an ending flight with an enemy within 8 m of the droid (`BOT_BLAST`) starts the destruct as it hands back (the
  countdown runs on, §7); otherwise the droid is left where it is until its 90 s run out;
- the droid shoots the nearest enemy it sees within 40 m (`BOT_RANGE`; closes to `OptimalRange`, backs off under half
  of it, fires within `MaxRange` with the yaw error under 0.15 rad), else flies at the nearest enemy the bot knows of
  within 60 m of the body;
- among 2 enemies within 8 m (`BOT_CROWD`), or among 1 with its health under 30 % (`BOT_LOW`), it starts the destruct
  and flies on until it goes off: BF1's orbital salvo or BF2's incinerator burst, whichever the class has.

**After**: control comes back to the body, which fights or walks to its objective as any bot does, and rests 30 s.

With these rules a carrier's body is rarely the free kill #58 describes. Bots now also target and shoot enemy droids
as the PS2's scan does (§7's #58 row).

**Measured** (`DROID_CENSUS=1 round_to_outcome 36000 <seed> bf2_cor1 bf1_kas2 bf2_nab2`, seeds 1 and 2; "exposed" =
an enemy within 20 m of the body, or one within 60 m in sight of it, at the throw):

| | before | after |
|---|---|---|
| launches per carrier-minute (6 rounds) | 0.80-1.40, 189 in all | 0.30-0.78, 81 in all |
| launches with the body exposed | 127 of 189 (67 %) | 15 of 81 (19 %) |
| carrier time spent flying (body standing) | 22-58 % | 7-27 % |
| body hits taken while flying | 448 | 17 |
| carriers killed while flying | 68 | 1 |
| droids ending in the destruct | 0 | 15 |
| `stuck_max` per round (ticks) | 480 360 360 300 420 360 | 300 420 360 300 360 360 |

The exposed launches left are the press-to-throw lag: the switch, the press and the 0.3 s `InitialSalvoDelay` after
the decision, while enemies move.

## 8. The HUD while flying (card DROIDHUD, 2026-10-28)

Disc values from the baked class chains; how the HUD uses them is `re-hud.md` §3 and `re-hud-map.md` §3.

- **Map icon** [V disc, D use]: the droid class (`*_weap_inf_remotedroid_ord`) authors `MapTexture troop_icon` and
  `MapScale 1.0` in both games (soldiers' `troop_icon` is 1.4). The droid is a controllable, so the units table
  (`FUN_003a5940`, re-hud-map §3) lists it with its class's map info [D: not captured]. The port draws every live droid
  with that icon in its team's colour, enemy ones only when detected. `[ours]` the player's own flown droid shows too.
- **Weapon rows** [V disc, D use]: the PS2 publishes the controlled entity's weapons (`FUN_003b3398`); the droid's are
  its WEAPONSECTION 1 blaster (channel 0, ammo 0 = endless) and WEAPONSECTION 2 destruct (channel 1, ammo 1, 1 round).
  BF2: blaster `HUDTag hud_vehicle_recoiless`, destruct `HUDTag hud_detpack_plunger`; the 1-player layout carries a
  MeshInfo for `hud_vehicle_recoiless` under both `player1weapon1mesh` and `player1weapon2mesh` and one for
  `hud_detpack_plunger` under `player1weapon2mesh`. `hud_detpack_plunger` is a flat card whose texture is a picture of
  the probe droid. BF1 (no HUDTag): blaster `IconTexture IMP_pistol_icon`, destruct `HUD_orbital_strike_icon` (rep's
  blaster also has `GeometryName rep_weap_inf_pistol`). The blaster shows the infinity ring and no count; the destruct
  counts 1 (ammo 1 x `RoundsPerClip` 1). Not checked against a PS2 frame of a flying droid.
- Sprint while flying: the owner's pad is cleared while the droid holds it (`Sim::droid_pad`) and the flight reads no
  SPRINT, so it does nothing (test `sprint_does_nothing_while_flying`).
- Pictures: `engine/artifacts/lead_tmp/droidhud/{bf2_nab2,bf1_tat2}.png` (`golden_hud::droid_hud_picture`).

## 9. The camera, the rows' icons and the map while flying (card DROIDCAM, maintainer playtest 2026-10-28)

| decision | tag |
|---|---|
| Third-person chase camera (`game::walk::droid_camera`): track centre = the disc's `EyePointOffset` (0.5 m) above the droid, view = the droid's aim pitched down by its `TiltValue` (5°), camera `DROID_CHASE` (0, 0, 2.2) from the centre in that tilted frame (straight back at eye height, as the soldier's stand section), so it swings round the droid as it pitches. The disc's `TrackOffset` (0, -0.25, 1.5 / 1.8) is read and not used; how the PS2 applies it is untraced. The world pulls it in and floors it as it does the soldier's (`settle_camera`, `camera_collide`) | OURS |
| The reticle is the droid's aim ray 1024 m out projected through the drawn camera (`WalkSim::reticle_y`), as the soldier's; the reticle pick is the same ray (`core/src/sim/input.rs`) | OURS (as the soldier, §10.4 of re-aim-assist) |
| The droid's model is drawn turned by π (the engine's models face +Z; the aim looks down -Z at yaw 0), so its blaster faces where it flies; checked by eye on the pictures, not against a PS2 frame | D |
| Weapon rows on EVERY map: blaster `hud_vehicle_recoiless`, destruct `hud_detpack_plunger` (BF2's HUDTags, from `common_bf2`, which the BF2 HUD loads on BF1 maps too); BF1's `IconTexture` sheets are not shown | OURS (maintainer: "the bf1 remote droid hud icons look poor") |
| Minimap while flying: centred on the droid, turned with its heading, its zoom from the droid's speed and the droid class's map view radii; the standing body shows as a soldier icon; on foot again, the camera and no own icon | OURS |
- Pictures: `engine/artifacts/lead_tmp/droidcam/{bf2_nab2,bf1_tat2}.png` (`golden_hud::droid_hud_picture`, now with the
  models drawn).
