# OPUS RE (2026-10-16) - [O] UNAUDITED

# RE: BF1 Naboo Plains' fambaas - the shield, the generator and the building (card FAMB)

Question (the maintainer, INBOX 2026-09-28, DECISION 1): BF1 Naboo Plains always has the Gungan fambaa and its shield. What is
the shield (what it blocks, its size, what drives it, what kills it), which missions load it, and what does the port
need to bring it into its NAB1 in every mode?

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`; the BF1 disc (`NAB1_H.LVL`, `NAB1.LVL`, `SIDE/GUN.LVL`, the mission
scripts); PS2 RAM of the historical battle, live and in save states. Tags: [V] read (B1 lines, function checked with
`re_check.py fn`), [D] derived, [H] hypothesis, [M] measured on the PS2 (PCSX2, the historical Naboo battle,
`nab1c_h`, 2026-10-16), [R] read from a PS2 save state. Load-bearing claims carry [O]. E = the building entity,
T = its MountedTurret, W = a weapon instance, wc = its class (W+0x64).

Hops: mission scripts -> the world's vehicle pads -> the two classes -> the label table (`commandarmedanimatedbuilding`)
-> the post component; TURRET1 -> the self-piloted turret tick (fixed aim, the LockOnRange ray, AutoFire) -> the two
weapons: WeaponShield (instance ctor, update, fire tick, predicate) and WeaponRepair (pulse, ammo feed) -> the base
weapon state machine (clip / reserve / reload) -> the shield's collision body (ellipsoid, layer 0x800) -> every
collision query's mask -> the bolt hit; the effect config. Three Opus readers (`lead_tmp/famb/AGENT_FIRE.md`,
`AGENT_LAYER.md`, `AGENT_CMD.md`), their load-bearing lines re-read by the lead and then measured live.

## 0. The answers

| question | answer |
|---|---|
| which missions | **Only the historical campaign's `nab1c_h`** ("The Battle of Naboo", Gungans vs CIS; the PS2's first Clone Wars campaign mission) loads `NAB\nab1_h.lvl`; the Instant Action / conquest missions `nab1c` (CW) and `nab1i` (GCW) load `NAB\nab1.lvl` (1) [V][O]. |
| where | Two pads of the regular `naboo1` world (same world in both LVLs): `perma_spawn10` (-4.00, -107.14, -77.80) spawns the SHIELD fambaa and `perma_spawn13` (-4.00, -107.14, -59.80), 18 m on, the GENERATOR fambaa - both `ClassHisDef`, Team 2, ControlZone CP1, SpawnTime 100000 (2) [V][O]. CP1 (the defenders' home post, (-5.1, -112.9, -123.4)) is 45.6 m from the shield fambaa: inside the dome. |
| what the shield is | A **60 x 30 x 60 m ellipsoid** (half-axes x, y, z) centred on the shield fambaa's origin (ShieldOffset 0): a dome 60 m in radius, 30 m high (3.4) [V][O]. |
| what it blocks | **Ordnance coming in from outside, nothing else.** Bolts / missiles / grenades sweep the shield layer; soldiers, vehicles, aim rays, AI sight rays and blast gathers do not. The test reports only an entry from outside, so shots from inside go out freely (3.5) [V][M][O]. A bolt that strikes it dies there and damages only the shield's points (flags 1); grenades bounce off its outside (3.5). Live: the CIS player walked from 75 m to 14.8 m through it; his bolts from 75 m never touched the fambaa's health, from 14 m inside they did (3.6) [M]. |
| what drives it | The shield fambaa's `shield` weapon fires itself (AutoFire on a self-piloted seat): each shot sets the shield to 100000 points, which drain at 100000 /s. It has 2 clips and nothing of its own to refill them. The GENERATOR fambaa's `repair` weapon, aimed backwards (yaw 180) along a fixed 50 m ray from its back, hits the shield fambaa and adds one clip (`BuildingAmmo 1`) every 0.1 s; the shield weapon fires each clip at once. So the shield is reset to 100000 every 0.1 s and never falls under ~90000 (4) [V][M][O]. |
| what kills it | Killing the generator (or anything that stops its feed): the shield fires what it holds and is empty and OFF about 1 s later, and stays off (no self-recharge: `AddShieldOff` 0). Live knock-out of `BuildingAmmo`: shield 0 and off at +1.001 s, still off 12 s later; back at 100000 0.067 s after the feed returned (4.3) [M][O]. Bolts alone cannot: 100000 points refilled every 0.1 s. Both fambaas stand inside the dome, so the generator must be shot from inside (or by blasts, 3.5). |
| the building | `commandarmedanimatedbuilding` = an armed animated building WITH the ordinary command-post component: a mobile spawn point for its team (6 spawn points, +-8 m beside it; live 16 of 30 Gungan respawns came out there), a map icon, no capture, ValueBleed and attack / defend values at the post class default 100 against the map posts' 10 (5) [V][M][O]. MaxHealth 10000, HealthType building; at 0 its turrets stop, the post dies, `fambaa_death` plays and the model leaves the scene; RespawnTime 6,000,000 s = never in a match (5.3) [V]. |
| the look | The `fambaa_shield` effect: a dome of `Radius 60`, `Height 30`, texture `fambaa_shield2` scrolled at 0.1, colour 255 255 255 255 and 240 160 255 (lavender) (6) [V]; PS2 frames `lead_tmp/famb/ps2/dome_out110.png`, `dome_out75.png`, `dome_in30.png`. The draw itself is not read [H]. |

**For the port:** none of this is in the port's NAB1 (its pads have no `ClassHis*` row and NAB1_H is not baked). Section
7 is the build split.

## 1. Which missions load the fambaas [V][O]

- `missions.py` (all 44 BF1 mission scripts, Lua 4.0 reader `scripts/gen_missions.py`): **only `nab1c_h`** reads
  `NAB\nab1_h.lvl`; `nab1c` and `nab1i` read `NAB\nab1.lvl` [receipt:2bfa41c9].
- `nab1c_h`: `SetHistorical()`; team 2 `Gungans` (gun_inf_soldier 17, gun_inf_rider 7, gun_inf_defender 8), team 1
  `CIS` (four battle droid classes), 32 units and 200 reinforcements each, `SetAttackingTeam(1)`. It asks
  `SIDE\gun.lvl` for `gun_inf_basic`, `gun_walk_kaadu`, `nab_bldg_fambaa_shield` - GUN.LVL answers that last name with
  an EMPTY sub-level (a `lvl_` chunk at 0x41110 holding only fnv(nab_bldg_fambaa_shield) and size 0) and defines no
  fambaa class; both fambaa classes come with `NAB\nab1_h.lvl` [receipt:87f083b8] (no other LVL defines one; two
  verifiers).
- The PS2's own menu: Historical Campaign -> Clone Wars -> "The Battle of Naboo": "Destroy the mobile shield
  generators by eliminating all the fambaas. Wipe out the entire Gungan Grand Army." (`lead_tmp/famb/ps2/m6.png`);
  in battle the objectives read "Capture enemy command posts / Destroy the fambaas" (`m9.png`) [M].
- `nab1c`: team 2 Republic, team 1 CIS, team 3 Locals = Gungans (gun_inf_soldier 5, gun_inf_rider 2), the Republic's
  friend; `nab1i` the same with Alliance / Empire. **No Instant Action or conquest mission has the fambaas.**

## 2. The pads and the classes [V][O]

- Both LVLs carry one world `naboo1` of 214 instances (`tools/pads.py`). The two fambaa pads exist in NAB1.LVL too;
  there their `ClassHisDef` names a class the level does not load, so they spawn nothing (`perma_spawn13` spawns a
  kaadu for the Republic / Alliance through `ClassRepDef` / `ClassAllDef`).
- The pad's class per team (`re-vehicle-pads-load.md` 1): with `SetHistorical` on, teams 1 and 2 resolve against the
  `ClassHis*` row, so team 2 (the Gungans, the pad's `Team` and CP1's owner) takes `ClassHisDef`. SpawnTime 100000 s:
  each fambaa is spawned once, in the post-load tick; a fambaa never decays at its pad (a self seat is its own pilot,
  `FUN_002afa90` B1:240116-240121) and is not respawned in a match [V].
- NAB1_H.LVL adds exactly the fambaa: model `nab1_walk_fambaa` (+ LOWD), skel, gmod, collision prim
  `p_collision12`, animation bank `nab_walk_fambaa` (zaa_ / zaf_; `fambaa_idle`, `fambaa_death`), textures
  `nab1_fambaa` / `_lowrez` / `_dtl` and `fambaa_shield2`, one fx__ (`fambaa_shield`), the two entc and two wpnc.
- The classes (`veh0/entc.py`; names by fnv, `BuildingAmmo` 0xa25f865f and `MuzzleFlashEffect` 0x39aaa863 found
  this pass):

| | `nab_bldg_fambaa_shield` | `nab_bldg_fambaa_generator` |
|---|---|---|
| BASE / body | commandarmedanimatedbuilding; GeometryName nab1_walk_fambaa, AnimationName nab_walk_fambaa, IdleAnimation fambaa_idle, DeathAnimation fambaa_death, MaxHealth 10000, RespawnTime 6000000, Label "Control Zone", MapTexture hud_dest_icon, MapScale 3, SpawnPointCount 6 (x +-8, z 0 / +-4, yaw 5 / 355), famba_moan / famba_hurt / famba_death | same |
| TURRET1 | PilotType self, Controllable 0, AutoFire 1, TurretNodeName hp_fire, Yaw / PitchLimits 0 0, MaxTurn / PitchSpeed 0, WeaponAmmo 2 | the same but YawLimits 150 210, PitchLimits -30 30, WeaponAmmo 0, `RestDir` 0 0 -1 (no reader [receipt:2bde4f59]) |
| weapon | `nab_weap_bldg_fambaa_shield`, class `shield`: RoundsPerClip 1, ShotDelay 0.1, ReloadTime 0, TriggerSingle 0, LockOnRange 0, MaxShield 100000, AddShield -100000, ShieldRadius 60 30, ShieldEffect fambaa_shield; no AddShieldOff | `nab_weap_bldg_fambaa_generator`, class `repair`: RoundsPerClip 0, ShotDelay 0.1, ReloadTime 0, TriggerSingle 0, LockOnRange 50, LockOnAngle 30, BuildingAmmo 1, MuzzleFlashEffect repairkit |

## 3. The shield [V][O]

### 3.1 The weapon (WeaponShield, BF1)
- Instance ctor `FUN_00293660` (B1:219029-219095): a collision body in layer **0x800** (B1:219043), its bounding
  sphere the largest ShieldRadius value (B1:219060-219071); the owner's shield max = MaxShield; the shield =
  MaxShield **only with infinite ammo** (B1:219081), else 0; rate = AddShieldOff; on-bit (E+0xac bit 4) clear. The
  fambaa's WeaponAmmo 2 is finite, so its shield starts at 0 and off.
- Update `FUN_00293920` (B1:219120-219310): off when the shield <= 0, when the owner's alive bit (E+0xac bit 3) is
  clear, or for a droideka in its ball states; otherwise **with finite ammo it is ON whenever the shield is above 0**
  (B1:219162); only with infinite ammo (the droideka) does a press toggle it. Rate = on ? AddShield : AddShieldOff;
  off releases the effect and removes the body; the on-bit is written back at B1:219306.
- Fire tick `FUN_00293f98` (B1:219354-219380): on the shot's first tick shield max = shield = MaxShield, on-bit set
  (B1:219371), rate = AddShield; one clip's worth is spent (W+0x98 -= wc+0x64).
- The "wants to fire" predicate 0x294080 (an export hole, disassembled; instance-vtable slot 0xa8 of 0x53cab0):
  0 with no trigger pointer, when overheated, or with infinite ammo (FLT_MAX at 0x53c9f4 = 0x7f7fffff); else
  TriggerSingle ? byte == 1 : byte > 0. AutoFire holds the byte at 2 .. 100 (`re-turret-ai.md` C22), and the fambaa's
  TriggerSingle is 0: it always wants to fire. ShotDelay has no reader for this class (the hash sits in five other setters
  only [receipt:c25c930a]): it fires as soon as a clip is loaded [V, absence verified twice].
- Drain `FUN_001bbbe0` (B1:64092-64125), from the building tick `FUN_001c3350` (B1:69034): shield = clamp(shield +
  dt x rate, 0, max). AddShield -100000: a full shield is empty 1.0 s after its last shot.

### 3.2 The fire cycle (base weapon, `FUN_00284318` B1:208357-208650) [V]
- WeaponAmmo counts CLIPS including the loaded one (turret setter `FUN_00225f08` B1:139120-139126; weapon ctor
  `FUN_002838d8` B1:207910-207924): W+0x90 reserve = n - 1, W+0x94 max = n, W+0x98 loaded clip = 1.0 (a fraction);
  an authored value < 1 means infinite.
- State W+0xcc: 0 -> fire while the predicate holds (B1:208454-208459); a fire that empties the clip -> 5 (reload) or,
  with the reserve under one round, 8 (empty) (B1:208499-208512); 8 waits for ammo (B1:208588). Reload moves
  min(reserve, 1) into the clip (`FUN_002854c0`); ReloadTime 0 is stored as 0.01 s (B1:210288). Both moves are
  skipped for an infinite reserve and for the player under the `/unlimitedammo` flag (`DAT_00519064`, B1:14135),
  where reload end just sets the clip to 1.0. The per-shot cost wc+0x64 is 1 / (RoundsPerClip + 0.0001) from the
  RoundsPerClip key (`AmmoPerShot` 0x222090f6, B1:209927-209934, only when authored, and key order decides);
  RoundsPerClip < 1 = a clip that never empties.
- So, alone, the shield weapon fires its two clips two ticks apart and then sits in state 8 for good [D].

### 3.3 Live [M][O] (`lead_tmp/famb/ps2/`, tools in `lead_tmp/famb/tools/`)
- Save state `pcsx2/ref/states/bf1_nab1h_spawnmap.p2s` (`ents.py`, `findturret.py`, `turret.py`): the two fambaas at
  E+0x230 = (-4.00, -107.46, -77.80) and (-4.00, -107.46, -59.80) (the pads dropped to the ground), vtable 0x53d418,
  health 10000; the shield fambaa's shield 96662.5 of 100000, rate -100000, flags 0x1c (alive + on); the generator's
  turret aims (0, 0, -1) from (-3.94, -96.85, -58.98) (its hp_fire, 10.5 m up) and its weapon's target is the shield
  fambaa; the shield weapon's max clips 2.
- `live1.csv` (450 game updates, 15 s, `sample.py`): the shield is reset to 100000 every 0.1 s (125 of 135 gaps;
  the rest 0.2-0.4 s), never under 91667, ON in 450 of 450; the generator targets the shield fambaa in 450 of 450.
- `reserve1.csv` (`reserve.py`): a 3-update cycle - state 8 with the reserve +1.0 (the pulse), 5 (reload), 1 (the
  shot; shield 100000, reserve -1.0).

### 3.4 The body: an ellipsoid [V][O]
- ShieldRadius is parsed "%f %f %f", a missing z = x and a missing y = x (`FUN_00294428` B1:219601-219602): "60.0 30.0"
  = 60 / 30 / 60 (live class words +0x200..0x208 = 60, 30, 60 [R]). The class finish `FUN_00294648` (B1:219712-219765)
  makes a kind-1 primitive - an ellipsoid (`FUN_001695b8` B1:7861-7868, `FUN_00169950` B1:8054-8058 -> the ray test
  `FUN_0016e030`) - named `shield`, at ShieldOffset (default 0, `FUN_00294178` B1:219459), placed each tick at the
  owner's matrix (B1:219261-219264). The fambaa's origin is on the ground, so: **a dome 60 m in radius and 30 m high**.

### 3.5 What crosses it [V][O]
- A collision query's mask selects layers bit by bit (grid entries carry their layer, `FUN_00199cc0`
  B1:40596-40605; walks skip `(mask & layer) == 0`, B1:39324 / 39603 / 39824). Only the shield body is ever in layer
  0x800 [receipt:80c246cd].
- **Not seen by:** soldier and vehicle movement (`FUN_00163008` B1:2692-2702: masks 5/8, 0x13/0xd, 6/8), the reticle
  and turret rays (0x5e / 0x4e; the default ray mask 0x5e, B1:1739), the AI's rays and gathers (0x4e, 0x48, 0x4c,
  0x1e, 0x21f, 0xc8, 0xc, 0x8) - the table of 89 query calls is `lead_tmp/famb/tools/calls.txt` (two of its rows
  are wrong: B1:115130 is mask 0x4c, B1:198940 is 0x812, a weapon fatray gather; the verifier checked 14 rows).
  So units and vehicles pass through, aim assist and the reticle see through it, and **bots see and target through
  it** (their bolts then strike it).
- **Seen by ordnance:** the bolt / missile sweep `FUN_00273128` (0x9de / 0x996, B1:195510-195514), the grenade
  `FUN_0027ae78` (B1:201148-201155), beacons `FUN_0027eff8` (B1:204160-204167).
- **One-way:** `FUN_0016e030` returns 1.0 (a miss) when the segment starts inside (B1:11873-11874) and otherwise
  reports only an inward crossing (B1:11906-11918): a shot fired inside the dome leaves it freely.
- A bolt from outside: the hit `FUN_00276810` damages the shield's owner with flags 1 = shield points only
  (B1:197809-197813, 197921; `FUN_001bc190` B1:64364-64385), and the bolt stops and explodes there (B1:195309-195340,
  195532, 2736-2738). Grenades bounce off the outside without damage [D].
- Blasts: the gather (0x81e, B1:194072; the fatray gathers 0x812 likewise) includes the layer but drops the shield
  body: gathers drop owner-0 bodies (`FUN_00162a08` B1:2371-2377) and the shield body's vtable (0x53ca58, set
  B1:219045) forwards only slots +0xc / +0x1c / +0x40 to the owner, leaving the owner slot +0x34 at the base's 0;
  and the blast's sight ray (0x48, B1:194099) ignores it: **splash crosses the dome** [D]. A rocket stopped at the
  surface still hurts what is within its blast radius inside.

### 3.6 Live [M]
- `cross1.csv` (every soldier at 4 Hz for 60 s, `cross.py`): Gungans crossed the 60 m line 26 times (20 out, 6 in)
  with the shield on in every row.
- `t1_*.csv` (`trial.py`, the CIS player kept alive): walked from 74.95 m to 14.80 m in 10 s, shield on throughout
  (the enemy walks in); 4 s of fire from 14 m inside took the fambaa's health 9850 -> 9748 (6-point hits: droid rifle
  damage x BuildingScale); 4 s from 75 m outside left it at 9748 in 178 of 178 rows, while 25 rows show 60-69-point
  dips in the shield's drain (the hits) against 4 in the walk's control rows.

## 4. The generator [V][O]

- The turret's fixed aim: yaw and pitch start at 0 and are CENTRED on the limits' middle when 0 lies outside them
  (`FUN_00222f20` B1:136679-136688), clamped every tick (B1:137468-137527); with MaxTurnSpeed 0 and no controller
  nothing turns them. YawLimits 150 210 -> yaw 180: the ray leaves the hp_fire node on its back along the building's
  local -z (B1:137821-137829) - 18 m behind it stands the shield fambaa.
- With `Controllable 0` and LockOnRange 50 the tick casts that ray every step and writes what it hits as the
  weapon's target, with no team or type test (B1:137839-137866); the turret tick runs every step from the building's
  creation to its kill (`FUN_00299490` -> `FUN_001c3238` -> `FUN_00225478` -> the update list `FUN_00197350`,
  B1:223626, 68966-68992, 138348-138377, 38756-38815, 328561-328563) - the point `re-turret-ai.md` 5 left open.
- The `repair` weapon (`WeaponRepair`, class ctor `FUN_00292e60`, setter `FUN_00292f60`): per target type
  (E+0xac & 7) a health and an ammo amount; only `BuildingAmmo` is authored. With ShotDelay > 0 it pulses once per
  ShotDelay at full amount (B1:218510, 218532; ShotDelay <= 0 would apply amount x dt every tick): **+1 clip every
  0.1 s** to every weapon of every turret of the target (`FUN_00292ce0` -> `FUN_002ab6a0` -> `FUN_002ab4d8`
  B1:237117-237167: reserve clamped to max clips minus a loaded one; infinite weapons unchanged). A type's bit in
  wc+0xf8 is set only by a non-zero amount. `MuzzleFlashEffect repairkit` is the pulse's particle effect.

### 4.3 The knock-out [M][O]
- `knock.py`: the generator class's `BuildingAmmo` (live word 0x1664750 + 0x210 = 1.0) written to 0: the shield read
  100000, then **0 and OFF at +1.001 s**, still off at +12 s (`knock1_off.csv`, 361 updates); written back to 1:
  100000 and ON at +0.067 s (`knock1_on.csv`). The generator alone keeps the shield up.

## 5. The building: a mobile command post [V]

- Label table `FUN_00172ab0`: `commandarmedanimatedbuilding` -> `FUN_002992c0` (B1:15615-15617) = the armed
  animated building class + the post component `FUN_002a0b28` (B1:223531-223545); the entity ctor makes its post
  `FUN_0029c788` -> E+0x1d24 (B1:223449-223450). The plain `commandpost` is the same component without turrets; so
  the fambaa is a command post the way BF1's command walkers are (`re-command-walker-post.md`).
- Spawns: SpawnPointCount / SpawnPointLocation are post keys (`FUN_002a1138` B1:229442-229613); its team may spawn
  there (`FUN_0029de80` B1:227152-227156). Live: 16 of 30 Gungan respawns in a minute came out exactly at x +-8,
  z 0 / +-4 of a fambaa (`cross1.csv`; the rest at the map's posts) [M]. A pad-built post gets no capture / control
  region and no SpawnPath (zeroed B1:225740-225743; only the instance setter fills them); its label is filled from
  the owner entity (`FUN_0029c970`). It stops being a spawn only when killed (`FUN_0029cc00`, unscheduled at
  B1:226321-226325). The factory can adopt an unowned post of the same class within 1 m (B1:226045-226065): none
  stands at the fambaa pads.
- Values: the post class defaults **ValueBleed 100** and **Value_ATK / Value_DEF 100** for every faction
  (B1:229234-229243); the fambaa classes author none; NAB1's five posts author 10 (`tools/cpvals.py`).
  - Bleed (`FUN_002ad830` B1:238680-238716): the team's sum is ValueBleed over its posts plus 1 per neutral post;
    the threshold scan (run when team+0x2c < team+0x10) sets no rate when the sum is above every threshold, so
    **while a fambaa lives its team never bleeds from posts** [D].
  - AI goals (`FUN_002618b8` / `FUN_002617f8`, B1:182069-182205): the attackers draw among the posts they cannot
    spawn at by Value_ATK: cp1, cp3, cp11 at 10 each and the two fambaas at 100 each - **about 87 % of the CIS's goal
    draws are a fambaa** [D]. What a bot does on arriving at a post with no capture region is not read [H].
- Damage (`FUN_001bc190`): any ordnance, Damage x its BuildingScale (HealthType building, B1:67215). At health 0
  (`FUN_001bfa88` B1:66652-66654): the turrets unlink and their weapons stop (the shield then decays, 4.3), the post
  dies (`FUN_0029cc00`), `fambaa_death` plays and the building leaves the scene (`FUN_001bfb88` B1:66705-66712);
  regen MaxHealth / RespawnTime = 0.0017 /s (B1:66848-66852): never back in a match.

## 6. The look [V] (the draw not read [H])

- The fx__ chunk `fambaa_shield` (NAB1_H) is a `Shield` config parsed by `FUN_0037bb18` (B1:368442-368640):
  `Radius` 60 -> rec+0x28, `Height` 30 -> +0x2c, `Texture` fambaa_shield2 -> +0x1c, `Color` 255 255 255 255 ->
  +0x20, key 0x01e4aaf7 240 160 255 -> +0x24, `ScrollRate` 0.1 -> +0x3c, 0xb28af6f4 0.8 -> +0x30, 0x08aab3aa 10 ->
  +0x34, 0xc8a9e3c6 2 -> +0x38, 0x5127f14d "fambaa" -> +0x18 = 1 (a shape flag: the string starts with f).
- PS2 frames (HW renderer, 2026-10-16): `dome_out110.png` (the dome's edge from 110 m: a translucent lavender shell
  over both fambaas), `dome_out75.png` (75 m), `dome_in30.png` (inside: the shield dishes on the fambaa's back, the
  shell behind). Save states beside them (`dome_out75.p2s`, `dome_in30.p2s`).
- Earlier note (DSHN audit): the ShieldEffect module draws through its own blend records (`FUN_00389338`: Cs x As +
  Cd, Cs x 0.5 + Cd, Cs + Cd) - the start for the look card.

## 7. For the port (the split)

Not in the port today: no `ClassHis*` row (`game/src/vehicle_pads.rs`), no NAB1_H bake, no shield of any kind
(`core/src/impact.rs`: "no shields in the port yet"). The maintainer's decision brings it into every NAB1 mode; the default
taken (card, reversible): the fambaas belong to team 2 - the Republic (CW) / the Alliance (GCW), CP1's owner, the
Gungans' friend - at the same two pads.
1. **FAMB (build)**: bake NAB1_H's fambaa classes, model, skeleton, idle / death clips and collision into bf1_nab1;
   spawn both at `perma_spawn10` / `perma_spawn13` for team 2 in every mode (the generator replaces that pad's
   Republic / Alliance kaadu, as in the historical layout); the buildings take damage (MaxHealth 10000, BuildingScale)
   and die; the shield rule (ellipsoid 60 / 30 / 60 at the shield fambaa, ordnance from outside only, shield points
   100000 drained 100000 /s, refilled by a shot per clip; the generator's fixed 50 m ray at yaw 180 feeding a clip
   every 0.1 s; off 1 s after the feed stops). Acceptance from section 3.3-3.6.
2. **FAMBV (re + build)**: the dome's draw (the effect config of section 6 and the ShieldEffect draw), against the PS2
   frames.
3. **FAMBP (needs the maintainer)**: the fambaa's post role outside the historical mission: spawning at it, the 100 / 100 / 100
   values (the attackers' goals and the bleed lock). Default until the maintainer answers: no post role in conquest.

## 8. Checks run, the verifier, what stayed [H]

- `re_check.py fn` on every cited line (the readers' and the lead's); `absent` receipts [receipt:2bfa41c9] (mission
  scripts), [receipt:87f083b8] (BF1 LVLs naming fambaa), [receipt:80c246cd] (layer 0x800 insert),
  [receipt:2bde4f59] (`RestDir`), [receipt:c25c930a] (ShotDelay: the 5 setters, no data word); `word` 0x53c9f4 = FLT_MAX;
  the predicate hole 0x294080 disassembled (`scripts/tools/eedis.py`).
- Live tools (`lead_tmp/famb/tools/`): `ents.py`, `findturret.py`, `turret.py` (states); `sample.py`, `knock.py`,
  `reserve.py`, `cross.py`, `trial.py`, `view.py` (a live session named `famb`); disc tools `pads.py`, `missions.py`,
  `cpvals.py`, `calls.py`, `luiscan.py`, `words.py`.
- Verifier: one workflow (`famb-re-verify`, 8 agents at medium effort): 6 batches of 6-8 claims to one verifier each,
  the 7 absence claims to two verifiers working apart (`lead_tmp/famb/VERDICTS.json`). **58 verdicts: 47 SUPPORTED,
  11 PARTLY, 0 NOT SUPPORTED**; every PARTLY folded in above (the respawn count 19 -> 16 and GUN.LVL's empty section
  re-run by the lead). Absence claims: 6 of 7 SUPPORTED by both verifiers; the second verifier overturned one wording
  (X2, "GUN.LVL holds no fambaa bytes": it holds the empty sub-level) - the claim now says what both support.
- Stayed [H]: the dome's draw; what a bot does at a post with no capture region; whether bots pick the building as a
  target; whether a repair reaches a destroyed building; the 0.85 x health pad-spawn hit (`FUN_002b0238`
  B1:240640-240654) and its flag; the spawn screen's use of the class Label "Control Zone"; whether the dome's body
  and effect leave at once when the SHIELD fambaa itself dies (the kill switches its weapon off before the update that
  removes them could run; next: read `FUN_00225520`'s weapon deactivate `FUN_0026dff8` for a shield release).
