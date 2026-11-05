# OPUS RE (loop lane 2, 2026-09-25, card VEH0) - [O] UNAUDITED: no Fable pass
# RE: vehicles on the PS2 — what the discs ship, the class code in both ELFs, the M2 build plan

> **AUDIT 2026-09-25 (loop audit of VENT, PARTLY — `engine/artifacts/loop/audits.md`).**
> - **Held:** the seat ring (§5), the PassengerSlots clamp and the one-pool / no-region reading (§6) re-derived and
>   verifier-SUPPORTED.
> - **Missing:** the vehicle's TEAM was never established. "The team comes from the pad (§7)" in §11 is not what §7
>   shows. The PS2 rule is §7.1: team 0 when empty, the last entrant's in ANY seat, command vehicles never neutral, the
>   pad's team kept apart.
> - **Qualified:** §6's "HealthType vehicle" is the class DEFAULT. The ODF overrides it (BF1 `all_walk_tauntaun` =
>   animal). Card VTEAM.

**The question** (card VEH0; PLAN §DESIGN SPECS "P7 DESIGN — entity shape", its seven RE questions in order): (1) the
hover / walker / flyer class properties and the integrators' PS2 addresses; (2) seats and enter/exit; (3) damage regions,
chunks and explosions; (4) vehicle spawn pads; (5) turrets as seats on buildings; (6) BF1 aim assist against vehicles;
(7) BF1 vs BF2 handling. Then the M2 build plan (§11).
**Method.** The lead read the class factories, the hover family and the port; four helper agents read walker/flyer,
seats, damage/pads and aim in parallel from a shared brief (`engine/artifacts/lead_tmp/veh0/CONTEXT_FOR_AGENTS.md`),
each writing tagged notes (`lead_tmp/veh0/notes_*.md`) with every cited line checked by `re_check.py fn`. **[O] marks
only claims the lead reproduced by re-reading the cited lines or the Opus verifier supported (§12);** a plain [V] in
§4–§9 is a helper's reading, line-checked but not re-read by a second reader. Tools, in `lead_tmp/veh0/`: `fnv.py` (the
engine's property hash: FNV-1a 32 over bytes OR 0x20), `hashloc.py` (every decomp line holding a property's hash, with
its function), `setter.py` (a SetProperty's hash → offset table), `elfword.py` (a value as a data word or a lui/ori pair —
tables and some literals are invisible in the decomp text), `elfstr.py`, `entc.py` (a class's PROPs from a LVL),
`brute.py` (CamelCase name search over a hash). **Property names are never strings in either ELF; every name here is
confirmed by fnv(name) == the constant.** Conventions: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 =
`analysis/SWBFRNT2.ELF-decomp.c` (lines); ELF offset = VA − 0x160000 + 0x1000; [V] read, [D] derived, [H] guess; [O] =
load-bearing.

## 0. The answers

| # | question | answer |
|---|---|---|
| 1 | classes, props, integrators | 53 (game, class) pairs on 25 maps (§1). **Hover** (19 + BF1's 2 commandhovers, `treaded` included) run ONE integrator — B1 tick FUN_001eba60 / spring FUN_001ee040 / drive FUN_001ec768; B2 FUN_0021ca80 / FUN_0021f5d0 / FUN_0021d8b8 — ray-cast spring points or a lift spring, gravity × GravityScale, **velocity matching** thrust (Acceleration / Deceleration / Traction), filtered yaw rate (§3). **Walkers** (11 + 4 command) move by the walk animation's **root motion**; **flyers** (15) share a 5-state machine (landed / takeoff / flying / landing / 4) and a clamped Min–Mid–Max speed law (§4). Property → offset tables and the identical constructor defaults are in §3.1 / §4. |
| 2 | seats, enter/exit | `FLYERSECTION` BODY = the pilot seat, TURRET1..8 = MountedTurret gunner seats, `PassengerSlots` ≤ 4 — a ring of ≤ 13 seats (§5). Only soldiers enter: the use input gathers objects within radius + 0.5 and picks what the aim cone (≤ 5 m + target radius, 45°) scores best; the first free `PilotType = vehicle` seat in ring order (driver first); instant; exit tries 9 points round the hull (10 m ground ray BF1, 15 m BF2). Same code in both games; BF2's Lua EnterVehicle calls the same virtuals. |
| 3 | damage, chunks, explosion | **One hull pool**, HealthType vehicle → the ordnance's VehicleScale; per-part data are only visual damage effects between health % bands; at 0 health the `_exp` class detonates and the `CHUNKn` pieces fly **in the same tick — no wreck**; no vehicle self-repairs (§6). |
| 4 | spawn pads | `com_item_vehicle_spawn` (336 on 25 maps): the class per team comes from a side table (**Atk = team 1, Def = team 2**), the pad follows its CP's owner, **SpawnTime = the respawn delay** (first spawn immediate), one vehicle per pad (BF2 `SpawnCount`), an idle vehicle decays after 20 s / 10 s at MaxHealth / DecayTime per second; `ExpireTimeEnemy` / `ExpireTimeField` are read by neither ELF (§7). |
| 5 | turrets on buildings | the SAME section/seat/enter code; an armed building has no pilot seat → **a turret = a vehicle with no movement and 1–8 gunner seats** (§8). |
| 6 | BF1 aim vs vehicles | BF1's magnet **does** target enemy vehicles (kind 3; `TargetVehicle` defaults on) and runs in any seat whose weapon has LockOnRange > 0; BF1's lock-on is a launcher dwell lock (LockTime); BF2 has ONE lock mechanism on foot and in every vehicle kind (§9). **Correction:** `re-aim-assist.md` §3's acquisition function is the MountedTurret's; the soldier's acquisition filters by team and kind (card AIMF). |
| 7 | BF1 vs BF2 handling | hover: same equations and defaults; turn blend by stick (BF1) vs by speed (BF2); BF2 adds boost; BF1 folds strafe into turn on no-strafe hovers (§3.3). Walker: BF1 walks at the animation's own speed (MaxSpeed is only the AI's), BF2 time-scales the animation toward MaxSpeed / BoostSpeed (§4.1). Flyer: BF2 caps speeds in one mode, adds boost, gates takeoff by height (§4.2). PLAN's "BF2 ground handling" lock costs a BF1 class only its stick-based turn blend and strafe fold. |

## 1. What the discs ship — the census (Q: which vehicles, per map)

Every vehicle on the 25 vehicle maps enters through a `com_item_vehicle_spawn` pad (`wrld` instance; class BASE
`vehiclespawn`); no map places a drivable vehicle directly. [V] from the baked manifests' instance props, keys resolved by
FNV-1a (`Team`, `Layer`, `ControlZone`, `SpawnTime`, `ExpireTimeEnemy`, `ExpireTimeField`, `DecayTime`,
`Class{All,Imp,Rep,Cis,His,Loc}{Atk,Def}` — `ClassHisAtk/Def` found by brute force over 3-letter side codes), class BASE
from `assets_baked/classes/<game>_*.ron` or, for map-local classes (`*`), the map LVL's own `entc`
(`python3 engine/artifacts/lead_tmp/veh0/census.py` from ROOT reproduces this table). `His` (the dropped BF1 historical
campaign) is left out; H hover (incl. treaded), W walker, F flyer, CW / CH commandwalker / commandhover.

| map | pads | GCW (All/Imp) | CW (Rep/Cis) |
|---|--:|---|---|
| bf1_bes1 | 28 | all_fly_xwing F, all_fly_ywing F, bes_fly_cloudcar F*, imp_fly_tiebomber F, imp_fly_tiefighter F | bes_fly_cloudcar F*, cis_fly_droidfighter F, cis_fly_maf F, rep_fly_gunship F, rep_fly_jedifighter F |
| bf1_end1 | 12 | imp_hover_speederbike H, imp_walk_atst_jungle W | — |
| bf1_geo1 | 10 | — | cis_fly_geofighter F, cis_tread_hailfire H, cis_walk_spider W, rep_fly_gunship F, rep_walk_atte CW |
| bf1_hot1 | 16 | all_fly_snowspeeder F, all_walk_tauntaun W, imp_walk_atat CW, imp_walk_atst_snow W | — |
| bf1_kas1 | 4 | all_fly_xwing F, all_fly_ywing F, imp_fly_tiebomber F, imp_fly_tiefighter F | cis_fly_droidfighter F, cis_fly_maf F, rep_fly_gunship F, rep_fly_jedifighter F |
| bf1_kas2 | 11 | imp_walk_atst W | cis_hover_aat H, cis_hover_stap H, cis_walk_spider W |
| bf1_nab1 | 21 | all_fly_xwing F, all_hover_combatspeeder H, gun_walk_kaadu ?, imp_hover_speederbike H, imp_walk_atst W | cis_hover_aat H, cis_hover_stap H, gun_walk_kaadu ?, rep_fly_jedifighter F, rep_hover_fightertank H |
| bf1_nab2 | 4 | all_hover_combatspeeder H, imp_hover_fightertank H | cis_hover_aat H, rep_hover_fightertank H |
| bf1_rhn1 | 5 | all_hover_combatspeeder H, all_hover_hovernaut CH, imp_hover_fightertank H, imp_walk_atat CW | cis_hover_aat H, cis_hover_mtt CH, rep_hover_fightertank H, rep_walk_atte CW |
| bf1_yav1 | 7 | imp_hover_speederbike H, imp_walk_atst W | cis_hover_aat H, cis_hover_stap H, rep_hover_speederbike H |
| bf1_yav2 | 8 | all_hover_combatspeeder H, imp_walk_atst W | cis_walk_spider W, rep_hover_fightertank H |
| tat1 | 16 | all_fly_xwing F, all_hover_combatspeeder H, imp_fly_tiefighter F, imp_hover_speederbike H, imp_walk_atst W, tat_hover_skiff H* | cis_fly_droidfighter F, cis_hover_aat H, cis_hover_stap H, rep_fly_jedifighter F, rep_hover_fightertank H, rep_hover_speederbike H, tat_hover_skiff H* |
| tat2 | 1 | imp_walk_atst W | cis_hover_aat H |
| bf2_dea1 | 2 | imp_fly_ai_trooptrans F | — |
| bf2_end1 | 12 | imp_hover_speederbike H, imp_walk_atst_jungle W | — |
| bf2_fel1 | 11 | all_hover_combatspeeder H, imp_walk_atst W | cis_tread_snailtank H, rep_walk_atte CW, rep_walk_oneman_atst W |
| bf2_geo1 | 21 | — | cis_hover_stap H, cis_tread_hailfire H, cis_walk_spider W, rep_hover_fightertank H, rep_walk_atte CW |
| bf2_hot1 | 20 | all_fly_snowspeeder F, all_walk_tauntaun W, imp_walk_atat CW, imp_walk_atst W, imp_walk_atst_snow W | — |
| bf2_kam1 | 3 | uta_fly_ride_gunship F | uta_fly_ride_gunship F |
| bf2_kas2 | 30 | all_hover_combatspeeder H, imp_hover_fightertank H, imp_hover_speederbike H, imp_walk_atst W | cis_hover_stap H, cis_tread_snailtank H, cis_walk_spider W, rep_hover_barcspeeder H, rep_hover_fightertank H |
| bf2_myg1 | 26 | all_hover_combatspeeder H, imp_hover_fightertank H, imp_hover_speederbike H, imp_walk_atst W | cis_hover_aat H, rep_hover_barcspeeder H, rep_hover_fightertank H, rep_walk_oneman_atst W, uta_fly_ride_gunship F, uta_fly_ride_gunshipmyg F |
| bf2_nab2 | 17 | all_hover_combatspeeder H, imp_hover_fightertank H | all_hover_combatspeeder H, cis_hover_aat H, rep_hover_fightertank H, rep_walk_oneman_atst W (+ locals: all_hover_combatspeeder) |
| bf2_pol1 | 11 | all_hover_combatspeeder H | cis_hover_aat H |
| bf2_uta1 | 13 | all_hover_combatspeeder H, imp_hover_fightertank H | cis_hover_aat H, rep_hover_fightertank H, rep_walk_oneman_atst W, uta_fly_ride_gunship F |
| bf2_yav1 | 27 | all_hover_bos_combatspeeder H, all_hover_combatspeeder H, imp_hover_fightertank H, imp_hover_speederbike H, imp_walk_atst W | cis_hover_aat H, cis_hover_stap H, re_hover_barcspeeder ?, rep_hover_barcspeeder H, rep_hover_fightertank H |

- [V] 53 distinct (game, class) pairs: 19 hover, 15 flyer, 11 walker, 4 commandwalker, 2 commandhover, 2 unresolved —
  `gun_walk_kaadu` (BF1 NAB1; the Gungan side is not in the classes bake — resolved by card KAAD, 2026-09-29: the NAB1
  missions read SIDE\gun.lvl, whose kaadu is a BASE walker the PS2 spawns on those pads, 3 live in NAB1 RAM) and `re_hover_barcspeeder` (BF2 YAV1; no class
  of that name on the disc — a typo the PS2 would not spawn [H: the pad logic's handling of a missing class]).
  - AUDIT 2026-09-30 (KAAD, OK; `reports/20260930_0645_audit_KAAD.md`). In both eras' RAM the kaadu sits in the
    team-2 slot of 3 of the 21 live VehicleSpawns. It spawns at the pad's x/z on the ground with the pad's rotation;
    the port matches to 1e-4 m. Six of the seven pads also key ClassHisDef = kaadu (historical missions only).
  - A class name that no loaded class answers stores a null class in the setter (B1 FUN_002af778 B1:239879-239891,
    verifier-checked; B2 FUN_00371990 B2:369495-369503, read). `re_hover_barcspeeder` occurs once in all of
    extracted/bf2, as this YAV1 ClassRepDef value.
- [V] No-vehicle maps: BF1 BES2, KAM1, RHN2; BF2 COR1, DAG1, MUS1, TAN1 (no pad instance in their manifests).
- [V] `ExpireTimeEnemy` / `ExpireTimeField` are authored on every pad but read by NEITHER ELF: `elfword.py` finds their
  hashes (0x98fcc969, 0x937e6a7d) neither as a data word nor as a lui/ori pair in either ELF, while `SpawnTime`,
  `DecayTime` and `ControlZone` are found (B1 0x2af7bc / 0x2af7c0 / 0x2af77c, inside FUN_002af778).
- [V] The port plays each map's GCW mission (`<key>_con.ron`) and, where it exists, CW (`<key>_con_cw.ron`)
  (`game/src/mission.rs` header).

## 2. The class plumbing (both games)

- [V] ODF `ClassLabel` → constructor: BF1 `FUN_00172ab0` (B1:15507), BF2 `FUN_00173d10` (B2:16308), each pairing a label
  string (read from the ELF) with a `new` of the class size and its constructor:

  | label | BF1 ctor (size) | BF2 ctor (size) |
  |---|---|---|
  | hover, treaded | FUN_001efdd8 (0x28d0) | FUN_00221a20 (0xff0) |
  | walker | FUN_0021cca8 (0x28e0) | FUN_00263160 (0xfd0) |
  | flyer | FUN_001dead8 (0x2740) | FUN_0020f098 (0x1260) |
  | carrier | FUN_001c65b8 (0x27e0) | FUN_001e9bf8 (0x1300) |
  | commandhover / commandwalker | FUN_0029aee8 / FUN_002a2968 | FUN_00353b18 / FUN_0035bd30 |
  | commandflyer | — | FUN_00352f40 |
  | vehiclespawn / vehiclepad | FUN_002b0a08(.,0) / (.,1) | FUN_003734c8(.,0) / (.,1) |
  | armedbuilding | FUN_001c4cd0 | FUN_001e8860 |
  | portableturret / defensegridturret | — | FUN_0022d980 / FUN_001ead90 |

  So a `treaded` class (hailfire, snail tank) IS a hover on both discs.
- [V] Each class's `SetProperty` is a binary-search tree of *(decompiler detail omitted)* branches; the vehicle setters:
  hover B1 `FUN_001f0648` (B1:101587) / B2 `FUN_002223a0`; flyer B1 `FUN_001df3f0` / B2 `FUN_0020fd58`; walker B1
  `FUN_0021d980` / B2 `FUN_00263f68` (located by `hashloc.py`: the hashes of LiftSpring / SetAltitude / MaxSpeed /
  LandedHeight sit in exactly these functions and the droid / soldier twins).
- Property names are hashed, never stored as strings in either ELF; every name in this doc is confirmed by
  fnv(name) == the constant in the setter (tools `lead_tmp/veh0/{fnv,hashloc,setter,brute}.py`).

## 3. Hover (Q1, and Q7 for ground vehicles) — 21 of the 53 classes

Hover + treaded (19 classes) and BF1's two commandhovers run ONE integrator. Addresses [V][O]:

| piece | BF1 | BF2 |
|---|---|---|
| physics tick (per-point springs, ground-factor decay) | thunk FUN_001f3230 (B1:103378) → FUN_001eba60 | FUN_00224de0 (B2:138694) → FUN_0021ca80 |
| one spring point | FUN_001ee040 (loop B1:98046-98051) | FUN_0021f5d0 (B2:132903) |
| input → drive | thunk FUN_001f2268 (B1:102724) → FUN_001ea9f8 → FUN_001ec768 (B1:97698) | FUN_002245d0 (B2:138269) → FUN_0021b470 → FUN_0021d8b8 (B2:132580) |
| world contact | thunk FUN_001f2600 (B1:102949) → FUN_001ee708 → damage FUN_001ee3e0 | twin not opened [H] |
| commandhover reuse | FUN_0029c0e8 → FUN_001eba60 (B1:225686), FUN_0029b278 → FUN_001ea9f8 (B1:225059), FUN_0029b610 → FUN_001ee708 (B1:225284) | no BF2 map ships one |

### 3.1 Properties (setter B1 FUN_001f0648 / B2 FUN_002223a0; class offset B1 / B2) [V][O]
SetAltitude +0x176c/+0x654 · GravityScale +0x1770/+0x658 · LiftSpring +0x1774/+0x65c · LiftDamp +0x1778/+0x660 ·
Acceleration +0x177c/+0x664 · Deceleration +0x1780/+0x668 · Traction +0x1784/+0x66c · ForwardSpeed +0x1788/+0x670 ·
ReverseSpeed +0x178c/+0x674 · StrafeSpeed +0x1790/+0x678 · SpinRate +0x1794/+0x67c · TurnRate +0x1798/+0x680 ·
PitchRate +0x17a0/+0x688 · TurnFilter +0x17b4/+0x68c · PitchLimits +0x17c0,c4/+0x698,69c · ThrustPitchAngle
+0x17c8/+0x6a0 · StrafeRollAngle +0x17cc/+0x6a4 · BankAngle +0x17d0/+0x6a8 · BankFilter +0x17d4/+0x6ac · LevelSpring
+0x17d8/+0x6b0 · LevelDamp +0x17dc/+0x6b4 · CollisionThreshold +0x17e0..e8/+0x6b8.. · CollisionScale +0x17ec..f4/+0x6c4.. ·
VelocitySpring +0x27ec/+0xe78 · VelocityDamp +0x27f0/+0xe7c · OmegaXSpring +0x27f4/+0xe80 · OmegaXDamp +0x27f8/+0xe84 ·
OmegaZSpring +0x27fc/+0xe88 · OmegaZDamp +0x2800/+0xe8c · NoRandomSpring clears +0x2804/+0xe90.
Setter lines (B1, each *(decompiler detail omitted)*): SetAltitude 101841, GravityScale 102251, LiftSpring 101687, LiftDamp
102241, Acceleration 101831, Deceleration 102228, Traction 102017, ForwardSpeed 102358, ReverseSpeed 101794, SpinRate
102365, TurnRate 101692, PitchRate 102332, TurnFilter 102369, BankAngle 101676, LevelSpring 101697, LevelDamp 101789,
VelocitySpring 102317, VelocityDamp 102153, AddSpringBody 102072, BodyOmegaXSpringFactor 102272.
- **Spring points** [V][O]: `AddSpringBody = "x y z r"` (B1:102072) adds a collision sphere and a 0x20-byte entry at
  class+0x2808+i·0x20 — at most 6 (`if (5 < count) return`; count class+0x28c8); the modifiers that follow apply to the
  LAST point: BodySpringLength (+0x280c), BodyVelocitySpringFactor / BodyVelocityDampFactor (+0x2810/+0x2814),
  BodyOmegaXSpringFactor / DampFactor (+0x2818/+0x281c), BodyOmegaZSpringFactor / DampFactor (+0x2820/+0x2824)
  (B1:102272 writes `count·0x20 + 0x27f8`, the last entry's +0x10). Names found by brute force over CamelCase words
  (`lead_tmp/veh0/brute.py`), each confirmed fnv == constant.
- BF1-only: PCTurnRate / PCSpinRate / PCPitchRate (used when FUN_00311250() is true). BF2-only: BoostSpeed +0xf6c,
  BoostAcceleration +0xf70, BoostFOV +0xf74, EnergyAutoRestore +0xf64, and ~19 unnamed hashes.
- **Constructor defaults are identical in both games** [V][O] (B1 FUN_001efdd8 B1:101257-101290, B2 FUN_00221a20
  B2:136636-136668): Deceleration 5, ForwardSpeed 20, StrafeSpeed 15, PitchLimits −0.5/0.5, SetAltitude 1, GravityScale 1,
  LiftSpring 5, LiftDamp 2, Acceleration 10, Traction 10, ReverseSpeed 10, SpinRate 3, TurnRate 2, TurnFilter 10,
  PitchRate 0.5, BankFilter 2, LevelSpring 10, LevelDamp 3, ThrustPitchAngle/StrafeRollAngle/BankAngle 0,
  CollisionThreshold 7.5/2.5/0, CollisionScale 1/0/0. BF2 adds BoostSpeed 0, BoostAcceleration 0, +0xf78 0.5; spring
  jitter defaults ON (+0xe90 = 1).
- **Addendum (card VCLS, 2026-09-25) — [V] worker-read, NOT through the verifier:** an ODF class is DERIVED from its
  base class object by a copy (B1 FUN_001f00e0 via FUN_001f1f08, B2 FUN_00221e08) that keeps the drive scalars
  (B1:101409 copies +0x1788 etc.) but sets VelocitySpring 15, VelocityDamp 5, OmegaXSpring 3, OmegaXDamp 1, OmegaZSpring
  3, OmegaZDamp 1 and the spring-point count 0 (B1:101527-101534; B2:136938-136944) — so those six defaults are the
  copy's, not the ctor's. Each `AddSpringBody` entry starts (FUN_001f2030, B1:102605): length = body radius × 0.5
  (`DAT_00517828`, sole writer B1:101892 = 0.5), velocity spring/damp factors 1, both omega DAMP factors 1, OmegaX
  SPRING factor −sign(z) and OmegaZ SPRING factor sign(x) of the point (0 inside ±0.1). The hover setter turns only
  PitchLimits from degrees (B1:101859-101860); CollisionThreshold / CollisionScale are three-`%f` reads that fill what
  the line gives. PilotType's value is hashed like a key; an unknown value leaves the default (B1:156013-156034).
  Consumed by `game::vehicle_class`.

### 3.2 What the integrator does (BF1 FUN_001ec768 / FUN_001ee040 lines; BF2 same unless §3.3) [V][O]
Entity fields (BF1): velocity v +0x5f0, angular velocity ω +0x600 (x, yaw, z), ground factor g +0x610, class +0x624.
1. **Ground factor g.** Without spring points (and SetAltitude > 0) a ray down of length (current) + SetAltitude·10
   (B1:98788) gives height h and normal n; g = clamp(1 − (h − SetAltitude)/(9·SetAltitude), 0, 1) (B1:98864-98870).
   With spring points, FUN_001ee040 sets g = 1 whenever a point is compressed (B1:99853) and the tick decays g by dt,
   floor 0 (B1:98053-98061).
2. **Gravity.** No springs: v.y += −9.8·(1 + (GravityScale−1)·g)·dt (B1:98962-98965). Springs: v.y +=
   −9.8·(GravityScale + (1−GravityScale)·g)·dt (B1:99143-99146) — the weighting of g is inverted between the branches.
3. **Lift (no springs, SetAltitude > 0, B1:98967).** f = 9.8·g + LiftSpring·(SetAltitude − h) − LiftDamp·(v·n); if
   f > 0, v += n·f·dt (B1:98967-98987).
4. **Spring point i** (FUN_001ee040): c = clamp(penetration / BodySpringLength_i, 0, 1), kept as the tick's max
   (B1:99821-99826); × (0.9 + 0.1·rand) unless NoRandomSpring (B1:99844-99850); bottomed out while falling:
   v.y ·= 0.5, v.x, v.z ·= 0.9, ω ·= 0.9 (B1:99827-99834); F = c·VSF_i·VelocitySpring − (1−c)·VDF_i·VelocityDamp·
   min(v·up, 0), v += up·F·dt (B1:99865-99876); ω.x += (c·OXSF_i·OmegaXSpring − (1−c)·ω.x·OXDF_i·OmegaXDamp)·dt, ω.z
   the same with the Z terms (B1:99879-99888).
5. **Level.** ω.x, ω.z decay by (1 − min(LevelDamp·dt, 1)) and are pulled upright by min(LevelSpring·dt, 1)
   (B1:98996-99027; spring branch B1:99147-99156).
6. **Thrust is velocity matching, not force.** s = g·max(0, up.y)·dt; tractionStep = Traction·s; accelStep =
   (Deceleration + (Acceleration − Deceleration)·m)·s, m = max(|throttle|, |strafe|) (B1:98724-98730, 99029-99034);
   forward target = throttle·(throttle < 0 ? ReverseSpeed : ForwardSpeed), v += F·clamp(target − v·F, ±accelStep)
   (B1:99035-99071); sideways: StrafeSpeed < 1e-4 → v += S·clamp(−v·S, ±tractionStep) (traction kills side-slip), else
   target strafe·(−StrafeSpeed) at accelStep plus a carve term (B1:99072-99139). A hover with no ground contact (g = 0)
   cannot thrust.
7. **Yaw.** An AI driver (entity[0xe2] < 0): rate = SpinRate + (TurnRate − SpinRate)·m (B1:99276-99279, 99290). A human:
   rate = PitchRate + (that blend − PitchRate)·entity[0xf0] (B1:99281-99284; PitchRate default 0.5), or, when
   FUN_00311250() (a net-play per-player table flag) is set, [+0x17a4] + ([+0x17a8] − [+0x17a4])·m (defaults 4.5 / 3.0,
   B1:99287-99290). Then ω.yaw += (−rate·turn − ω.yaw)·min(TurnFilter·dt, 1) (B1:99294-99298); orientation integrates
   ω by an axis-angle quaternion, sincos(|ω|·dt·0.5) then a quaternion product (B1:99322, 99327).
8. **Visual tilt.** pitch += ((throttle·ThrustPitchAngle + k·BankAngle) − pitch)·BankFilter·dt, roll += ((strafe·
   StrafeRollAngle − k′·BankAngle) − roll)·BankFilter·dt (B1:99449-99457), applied as body rotations and removed at the
   next tick's start (B1:98720-98721).
9. **Inputs** (BF1 FUN_001ea9f8): throttle entity[0xc0], strafe [0xc1], turn [0xc2] (B1:97634-97635, 97698). A human
   driving a hover with StrafeSpeed == 0 has strafe folded into turn: turn = turn·(1 − |strafe|) + strafe, strafe = 0
   (B1:97668-97684) — on a BF1 speeder bike the stick's X turns. After the drive the aim angles clamp to the class limits
   (+0x17b8/+0x17bc yaw, PitchLimits pitch; B1:97725-97738).
10. **Contact** (FUN_001ee708): a hull contact removes v's normal component and adds a horizontal-only rebound (y zeroed)
    of … no text writers) when the normal speed < 0 and entity+0xac bit 3
    is set (B1:100124-100151); a spring sphere's contact is ignored within that point's BodySpringLength
    (B1:100116-100121); impact damage (FUN_001ee3e0, header B1:99903): threshold t = CT0 + CT1·(n·up) + CT2·(n·fwd),
    damage = (s + t)²·(CS0 + CS1·(n·up) + CS2·(n·fwd)) when s < −t (B1:99947-99954), times a factor from the other body
    (B1:99955-99959), dealt through FUN_001bc8e0 (B1:100017).
- Literal bytes [V]: −9.8f ×2 and 9.8f ×1 as lui+ori in both drives (B1 0x1ec8e0 / 0x1ed0a8 / 0x1ed0f0, B2 0x21dad8 /
  0x21e28c / 0x21e2d8); 10.0f and 9.0f as lone lui (B1 0x1ecc3c / 0x1ece38, B2 0x21de44 / 0x21e038).
- Not traced [H]: where pos += v·dt and the world sweep run (a shared dynamic-body step), the carve term's exact shape,
  what entity[0xf0] is, and the B2 contact twin — HVRE's job.
- **→ Answered by `analysis/re-hover.md` (HVRE, 2026-09-26, [O]).** That doc is the statement-level BF2 transcription
  and supersedes this section for the build:
  - the drive integrates the position and orientation itself;
  - E+0x360 is the controller's full-stick timer;
  - the B2 contact is FUN_0021fd98.

  It also corrects two points here:
  - BF2's TurnFilter is a **slew rate** (item 7's filter form is BF1's);
  - the orientation is integrated from the **tilted** matrix, so item 8's tilt is not "removed at the next tick's
    start" — it stays in the body (both games).

### 3.3 BF1 vs BF2 ground handling (Q7) [V][O]
- **Same:** property set and layout (every movement prop at the same relative offset), constructor defaults, the
  gravity / lift / level / thrust equations (B2 FUN_0021d8b8: gravity B2:133932-133936, lift B2:133945-133948, level
  B2:133971-133990, accel/traction B2:133995-134010), the spring point (B2 FUN_0021f5d0, plus a class flag +0xe76 gating
  the ray).
- **Different turn blend:** BF1 blends SpinRate→TurnRate by stick magnitude m (B1:98730, 99276-99290; m not reassigned
  in between); BF2 by forward speed, clamp(v·F / ForwardSpeed, 0, 1) (B2:134352-134372; a human's branch then mixes in
  PitchRate by entity+0x360 as BF1 does, B2:134376-134379). A BF2 hover turns at SpinRate when stopped and TurnRate at
  speed whatever the stick does.
- **Boost (BF2 only):** the boost flag (the byte at entity+0x1ddc, *(decompiler detail omitted)* on an int pointer) is set for a human
  with throttle > 0.9 and either the entity[0xaf] bit-0 input (sprint [H]) or the flag already on (B2:132519-132526), and
  for an AI when entity[0xc9] != 0 and a mode value 3 (B2:132540-132545); accelStep then uses BoostAcceleration
  (B2:133997-134002), the forward target BoostSpeed, and the turn rate scales by (1 − f·(1 − 0.5)) between ForwardSpeed
  and BoostSpeed (B2:134383-134397). BoostSpeed defaults to 0, so a class without it (every BF1-disc class) never boosts.
- **No strafe→turn fold in BF2** (FUN_0021b470 holds no `ABS(` and no StrafeSpeed read, B2:131921-132712; its drive
  zeroes the strafe input when StrafeSpeed < 0.0001, B2:133653): a BF2 speeder bike turns only from the turn input.
- **Aim-angle integration:** BF2 multiplies by 30·dt (B2:132578-132579), BF1 does not (B1:97659-97660) [D: resolve in
  HVRE].
- [D][O] PLAN's lock "ground vehicle handling = BF2" costs little: run the BF2 integrator on every class; a BF1-disc class
  loses only its stick-based turn blend and the strafe→turn fold (a feel choice to record in FEEL_NOTES when built).

## 4. Walker and flyer (Q1, Q7) — 15 walker-family and 15 flyer-family classes

### 4.1 Walker: movement IS the walk animation's root motion
- [V] **Setter** B1 `FUN_0021d980` / B2 `FUN_00263f68`: MaxSpeed +0x17b8/+0x748 (B1:133719, B2:183447; B1 default 20.0),
  MaxTurnSpeed +0x17bc/+0x75c, MaxYawSpeed +0x17c0/+0x760, MaxPitchSpeed +0x17cc/+0x76c, StoppedTurnSpeed +0x2620/+0x778
  (B1 default 0.1), ForwardTurnSpeed +0x2624/+0x77c (0.03), TurnThreshold +0x2628/+0x784 (0.3), MaxTerrainAngle
  +0x262c/+0x788 (0.9), LegRayHitLength +0x17b0/+0xe38 (2.5); BF2 only: BoostSpeed +0x74c, BoostTurnSpeed +0x780,
  JumpHeight → +0x750 = sqrt(19.6·h) (B2:183645-183652), EnergyAutoRestore / EnergyBoostDrain / EnergyCostJump
  +0x7a0..+0x7a8. BF1's ODFs spell the acceleration prop `Acceleraton` (0x9f8a0040) and BF1's walker/flyer setters accept
  only that spelling (B1:133833); BF2 maps both spellings to one field (B2:182876, 183624). `MaxStrafeSpeed` has no branch
  in either walker setter though BF1 ODFs author it.
- [V][O] **BF1 tick:** control FUN_00219ed8 (thunk FUN_0021f910) integrates the aim angles by MaxYawSpeed/MaxPitchSpeed
  (B1:130720-130733) and calls locomotion FUN_00218fa0 (vtable 0x533b18 slot +0x110): turn in place when |fwd|·5 ≤ |turn|
  (B1:129697), heading += turn·dt·StoppedTurnSpeed (B1:129699), the walk animation advances by the real dt and the body
  velocity = the root bone's displacement ÷ dt along the forward axis (B1:129878-129882). Legs FUN_0021ae90: per-leg foot
  placement, body pitch from the leg height difference clamped ±0.55 rad, gravity −29.4 m/s² when unsupported
  (B1:131315, 131361). **MaxSpeed and MaxTurnSpeed are read only by the AI** (FUN_0021c180 → the control inputs,
  B1:132205-132213); Acceleraton and ForwardTurnSpeed are read by no walker entity code.
- [V][O] **BF2 tick** FUN_0025d5e0 (vtables 0x61f980 walker, 0x62bf70 commandwalker): turn in place when class+0xe10·|fwd|
  ≤ |turn| (B2:178080); turn rate Stopped / Forward / BoostTurnSpeed (B2:178131, 178217, 178185); speed = MaxSpeed, or
  BoostSpeed with the stick forward (stick² > 0.81) and energy (B2:178132-178185); **the animation clock is scaled toward
  that speed** in two separate branches: dt·(speed / animSpeed) when the animation is slower, inside
  `if (entity[0x864] & 0x80)` (B2:178447, 178466; otherwise dt unchanged), and dt·max(0.5, speed·|stick| / anim) under
  *(decompiler detail omitted)* (B2:178489, 178532-178537; DAT_005fd240 = 0.5, no writers) — which branch a player's
  walker takes per tick is VWLK's to transcribe.
- [V] **commandwalker** = the walker class + a command-post component at +0x28e0 / +0xfd0 (SpawnPointCount,
  SpawnPointLocation, ValueBleed; B1 FUN_002a2968 → FUN_002a0b28, B2 FUN_0035bd30 → FUN_00359df0); its entity vtable
  reuses the walker's functions, so an AT-AT / AT-TE moves exactly like a walker and is also a mobile command post.
- **→ Answered by `analysis/re-walker.md` (VWLK, 2026-09-29, [O]).** That doc is the statement-level BF2 transcription
  and supersedes this section for the build. It corrects three points here:
  - the `& 0x80` clock branch is the **jump**, and it is dead on the discs: no walker class authors JumpHeight;
  - the walk is the other branch, dt·max(0.5, speed·|stick| / walk-clip speed). The live PS2 measured it at ×0.9000;
  - the heading step is a half angle, so the body turns by twice it: 2·TurnSpeed rad/s at most, in both games.

  The BF1 in-place test is at B1:129696.

### 4.2 Flyer: one 5-state machine and a clamped speed law
- [V][O] **Setter** B1 `FUN_001df3f0` / B2 `FUN_0020fd58`: Acceleraton +0x1778/+0x82c, MinSpeed +0x177c/+0x834, MidSpeed
  +0x1780/+0x838, MaxSpeed +0x1784/+0x83c, StrafeSpeed +0x1788/+0x844, PitchRate/TurnRate +0x178c,+0x1790/+0x848,+0x84c,
  PitchFilter/TurnFilter +0x179c,+0x17a0/+0x850,+0x858, ThrustPitchAngle, StrafeRollAngle, BankAngle/BankFilter,
  LevelFilter, TakeoffHeight/Time/Speed +0x17b8..c0/+0x888..890, LandingTime/Speed +0x17c4,c8/+0x894,898, LandedHeight
  +0x17cc/+0x89c; BF2 adds RollRate +0x880, BoostSpeed +0x840, BoostAcceleration +0x830. SetAltitude is NOT a flyer prop.
- [V][O] **Integrator** B1 FUN_001d9180 (B1:85041-86725; thunk FUN_001e0858) / B2 FUN_00202d08 (B2:116468-119660): state
  entity[0x186] (B1) with a 5-way jump table at 0x530650 — 0 landed, 1 takeoff (a 1024 m ray down; lift fraction +=
  dt/TakeoffTime, B1:85679; at 1 → flying), 2 flying, 3 landing (fraction −= dt/LandingTime; touchdown needs ground
  normal·up > 0.9375, B1:85765), 4 fraction −= dt·0.2 [H: crashing]; the toggle is one input byte (B1 +0x2ee).
- [V][O] **Speed law** (B1:85806-85830): target = Mid + t·(Max − Mid) for throttle t ≥ 0, Mid − t·(Min − Mid) below; the
  forward speed is clamped to within ±Acceleraton·dt of the current forward velocity; strafe likewise toward
  StrafeSpeed·input. Rates are first-order filters (PitchRate/PitchFilter, TurnRate/TurnFilter; B1:85872-85877,
  86027-86032).
- **→ The parked flyer's height is read in `re-vehicle-rest.md` (VREST, 2026-10-07, [O]):** the constructor adds
  `LandedHeight` (authored, else minus the model's vis-box min y) to the pad's spawn point; landed, nothing moves it;
  BF1's takeoff waits for LandedHeight + TakeoffHeight too.
- [V][O] **Terrain:** flyers are kept in a world altitude band set only by Lua `SetMin/MaxPlayerFlyHeight` (defaults
  −200 / +200 m, B1 FUN_002325f8 B1:147426-147427); near the band the pitch command is overridden (B1:85974, 85981).
  Takeoff and landing use rays; other terrain contact is ordinary collision [H].
- [V] **carrier** (BF1 rep_fly_vtrans) = a flyer that snaps each cargo to its matrix and velocity every tick (B1
  FUN_001c60b0 runs FUN_001d9180 then the cargo, B1:71252; CargoNodeName B1:71603).
- [V] **BF1 vs BF2 flyer:** same skeleton and states; BF2 caps Max ≤ 60 / Mid ≤ 20 / Min·0.2 ≤ 10 while entity+0x668 ≠ 0
  (words 0x5fcf24.. = 60, 20, 10, 0.2; the flag's writer not found [H]), adds boost (BoostAcceleration while boosting,
  ×3 deceleration), and advances takeoff only above TakeoffHeight (B2:117870-117872).

## 5. Seats, enter and exit (Q2) — the same machinery in both games

- [V][O] **Sections are gunner seats.** `FLYERSECTION` hashes its value: TURRET / TURRET1 → 0, TURRET2..TURRET8 → 1..7,
  anything else (BODY) → −1 (B1 FUN_001f0648 B1:102492-102537; B2 FUN_002223a0 B2:137087-137140). Props after BODY go to
  the vehicle's own class = the pilot seat; a section index allocates a MountedTurret class (B1 0x11b0 bytes,
  FUN_00225cd8, class+0x2678+4i, 8 slots; B2 0x8a0, FUN_00270540, +0xc68) whose setter B1 FUN_00225f08 / B2 FUN_00270770
  takes turret props (Yaw/PitchLimits deg→rad, turn rates, turret sounds, up to 4 weapons with WeaponAmmo < 1 =
  infinite), then the camera (EyePointOffset), then the aimer, then the seat (Controllable) props.
- [V][O] **Seat props** (Controllable setter B1 FUN_0023da58 / B2 FUN_002f6eb8): PilotPosition → the hardpoint node
  (+0x1c4, B1:156093), PilotAnimation / Pilot9Pose → the posed pilot animation (+0x210), **PilotType** → +0x21c: none 0,
  self 1, vehicle 2, remote 3 (B1:156006-156035); BF2 adds vehicleself 4 and defaults to 2 (B2:284987); NoEnterVehicles,
  FirstPersonFOV / ThirdPersonFOV, VehiclePosition.
- [V][O] **Passengers:** `PassengerSlots = N` allocates one PassengerSlot class, N clamped to 0..4 (B1:102135-102150; B2
  B2:125348-125362); no weapon, a camera and a seat only. The entity builds its seat ring in order: the vehicle's own
  seat (pilot), one MountedTurret per section in index order, then the passengers (B1 FUN_001d76a0 B1:83870-83944) — at
  most 1 + 8 + 4 = 13 seats [D]. MountedTurret and PassengerSlot are both entity kind 7 (RE_LOG FUN_00234c00).
- [V][O] **Who can enter:** only soldiers — the four seat virtuals (CanEnter / Enter / SwitchSeat / Exit) are real only in
  the soldier's Controllable vtable (B1 0x532be0 +0x48..+0x54 → FUN_0020b1b0 / FUN_0020b2c8 / FUN_0020b5e8 / FUN_0020b918;
  B2 0x61e998 +0x64..+0x70 → FUN_0024a9d0 / FUN_0024ac98 / FUN_0024b4d0 / FUN_0024ba68); every other class returns 0
  (B1 FUN_0023e7c8 = `jr ra; move v0,zero`). BF2's Lua `EnterVehicle` / `ExitVehicle` call the same slots and accept only
  EntityWalker / EntityHover / EntityFlyer targets (B2:35544-35589). BF1 has no binding but the same four virtuals.
- [V] **A BF2 pilot cannot leave a flyer in flight** (B2 FUN_00202d08, `re-space-assault.md` §10): the exit button reaches
  the exit logic only when landed. FUN_0024ba68 itself has no refusal and a gunner seat has no such gate. BF1 not read.
- [V][O] **Player path and range** (B1 FUN_0020a4f0 / B2 FUN_00249870, same code): on the use input (controller byte +0x52
  from bit 7 of the mapped action word, B1:180043; the physical button [H]), gather ≤ 16 objects within the soldier's
  collision radius + 0.5 (B1:119520), then score each with FUN_00241b98(targetR, 1.0, 5.0, eye, aim dir, centre)
  (B1:119544): t = (centre − eye)·dir, accept 0 ≤ t ≤ 5.0 + targetR and perp² ≤ (targetR + t)², score t + 16·perp²,
  lowest wins (B1 FUN_00241b98 B1:158844-158895). So you enter what you look at within 5 m + its radius, in a 45° cone.
- [V] **Seat choice** (CanEnter B1 FUN_0020b1b0): target +0xac bit 3 set, the team open (< 1) or matching for an AI (a
  player goes through FUN_00221890 [H]), and a seat in ring order with PilotType 2 that is empty or AI-held → the driver
  seat first when free [D]; a player takes over an AI-held seat and the AI is exited and placed (B1:120133). BF2 also
  refuses a flyer in state 2 (B2:164851) and accepts vehicleself seats.
- [V] **Switch seats:** FUN_0020b5e8 walks the ring to the next free type-2 seat, skipping passenger→passenger moves.
- [V][O] **Exit placement** (B1 FUN_0020b918): 9 candidates at the vehicle's collision radius r in its yaw frame — behind,
  left, right, the four diagonals (0.7071·r, lui+ori at 0x20bae8, B1:120506), front, then the centre forced (candidate 8
  skips the test); each needs a clear path from the centre (B1:120615), a 5.0 ray up (y += 5t − 0.1, B1:120618-120623)
  and a 10.0 ray down that hits (y −= 10t, B1:120624); then velocity 0, stance 0 (B1:120721), collision back. BF2
  FUN_0024ba68: the same 9 candidates with a 15.0 down ray (0x41700000, B2:165844) and a best-height pick [H].
- [D] **No timer:** Enter and Exit complete inside one call (no timer read in FUN_0020b2c8 / FUN_0020b918).
- [V] **AI boarding** (locations only): BF1 FUN_00244848 → FUN_0023c418 queues a request consumed by FUN_0023d418, which
  calls the same soldier Enter slot (B1:155554); BF2 AI commands 0x13/0x14 in FUN_0027bae0 / FUN_0027caf0.
- **VSEAT addendum (2026-09-28, Opus verifier 6/8 SUPPORTED, 2 PARTLY — folded in below) [O]:**
  - [V] The exit's 9 candidates (FUN_0020b918, stack block B1:120497-120530, stores 0x20bab8-0x20bb54), (x, y, z):
    (0, 1, −ρ), (−ρ, 1, 0), (ρ, 1, 0), (−kρ, 1, −kρ), (kρ, 1, −kρ), (kρ, 1, kρ), (−kρ, 1, kρ), (0, 1, ρ), (0, 1, 0),
    k = f32 0x3f350481 = 0.7071 (0x20bad4-0x20baec). Only index 8 skips the path test (`beq s5, 8` at 0x20bc94).
  - [V] ρ = R + (DAT_00533090 − 0.15)·1.2 (0x20ba08-0x20ba6c; the decomp's folded `+ 0.6840001`, B1:120495): R is
    the sphere radius FUN_001645a0 returns for the entity the seat hangs on; DAT_00533090 = f32 0.72, no writer in
    the text (3 hits, all reads), the first of a {0.72, 0.45, 0.80} table [H: a soldier radius per stance].
  - [V] Up ray: direction … down ray −up,
    taken only when t < 1.0 (y −= 10t), then break. All 9 failing leaves the frame's origin (row 3, the position).
  - [V]/[H] The frame (FUN_001ac6f8, 0x20bba4-0x20bc28): row 1 = the entity's normalised +0x10 row (its UP — the
    candidates' y = 1 rides it), row 0 = up × b, row 2 = b made perpendicular to up, row 3 = position; b = the +0x20
    row of another virtual's matrix ((a local+0x294)->vtbl+0x28) [H: the vehicle's forward, so candidate 0 is
    behind]. The port uses world up and the heading (PROVISIONAL).
  - [V] With argument 2 != 0 (a seat change), the last seat of type 2 whose occupant's +0xe8 < 0 [H: an AI] is moved
    into the seat left (B1:120689-120718: FUN_0023c488 ×2, FUN_00296df8).
  - [V] FUN_00241b98 (B1:158847-158897, disassembly 0x241b98-0x241c84): perp² is 3-D for flag ≠ 0 and x-z only for
    flag 0; score t + 16·perp² (`lui 0x4180`), else −1.0. FUN_0020a4f0's literals: 0.5 `lui 0x3f00` 0x20a598, 1.0
    0x20a9d4, 5.0 0x20a9e0; a candidate is scored only if `(self+0x534)->vtbl+0x48` (CanEnter) accepts it; an object
    with a non-null vtbl+0x38 is first box-tested against the soldier's centre padded by its radius + 0.5 [H: which
    objects]. Port: `core::sim::seats`.

## 6. Damage and destruction (Q3) — one hull pool, no wreck

- [V][O] **Health type:** every vehicle constructor stores HealthType 3 = vehicle (B1 hover B1:101255, flyer B1:89246,
  walker B1:132738; B2 FUN_00221a20 / FUN_0020f098 / FUN_00263160 at class+0x2e4); the entity copies `HealthType & 7`
  into its flags (B1 FUN_00221200 B1:135467), which index an ordnance's per-type scale — so a vehicle takes
  `VehicleScale` (re-blaster-damage.md §6). ODF `HealthType` values person/animal/droid/vehicle/building → 0..4
  (B1:136172).
- [V][O] **One hull pool:** one health component per vehicle entity; no region or part-health property in either damageable
  setter (B1 FUN_00221e48, B2 FUN_0026a340). The only per-part data are up to 10 visual damage slots:
  `DamageStartPercent` opens a slot (B1:136004), `DamageStopPercent` (B1:136046), `DamageEffect`, `DamageAttachPoint`,
  `DamageEffectScale` (default 1.0), `DamageInheritVelocity`. The disc reads as "play the effect while health is between
  Stop and Start": speeder bike (600 HP) sparks 60→20 %, smoke 30→0 %, flame 2→0 % [D; the runtime reader not found, H].
- [V][O] **At 0 health:** the alive bit (flags bit 3) clears (B1:64610); the hover tick then, in the same frame, spawns
  the `ExplosionName` class at the vehicle (B1:97306, when entity[0x852] == 0), throws the chunks (FUN_001e2630,
  B1:97314) and removes the vehicle (the vtable at entity+0xb0, +0xc; B1:97315). **No wreck persists**; no vehicle parses
  or carries `DestroyedGeometryName`. BF2 the same (FUN_0021b470 B2:132031, 132040). Falling below y = −500 also removes
  it (B1:97338).
- [V] **Chunks** (`CHUNKSECTION` CHUNK/CHUNK1..CHUNK9 → 9 slots; setter B1 FUN_001e3678 / B2 FUN_00214600; spawn B1
  FUN_001e2630): v = vehicle velocity × ChunkVelocityFactor + outward dir × ChunkSpeed, tilted up by ChunkUpFactor; if
  |v| < 5 a random 4..6 m/s; v.y ≥ 2..3 (B1:91931-92121); defaults bounciness 0.6, stickiness 0.7 (B1:93002-93007).
  Disc: speeder bike 5 chunks, AT-ST 4 (speeds 10/9/6/5); the `_exp` class carries the blast (AT-ST 50 damage, radius
  20; speeder bike 20 / 10). Chunk lifetime [H]. **Answered by `re-vehicle-death.md` (card VDTH, 2026-09-28):** the
  blast's falloff, LOS and team rules (§2-§4), and the chunk launch, body and lifetime (§6). One correction to the
  launch above: with ChunkSpeed unset (default −1) the outward kick is U(15, 30) divided by the vehicle's
  collision-sphere radius (§6.1 there; the VEH0 helper notes' "/ mass" is wrong).
- [V][O] **Occupants:** at death FUN_001ea190 calls vtable +0xc on each seat object (B1:96789): every rider is
  **killed** with the vehicle's last-damage record, so the destroyer is credited — `re-vehicle-death.md` §10 (card
  RIDK, 2026-10-16; BF2 the same; live on BF1).
- [V][O] **Collision damage:** §3.2 item 10 (hover); impact speed beyond CollisionThreshold, squared, × CollisionScale.
- [V] **Repair:** `AddHealth` would regenerate (B1 FUN_001bbbe0 B1:64106) but no vehicle on either disc carries AddHealth
  or shields. `com_item_vehiclerecharge` authors VehicleHealth 400 / PowerupDelay 1.0 (parsed B1 FUN_002aa820
  B1:236557, 236605) and the fusion cutter's weapon class parses VehicleHealth (B1 FUN_00292f60) — their runtime [H].

## 7. Vehicle spawn pads (Q4)

- [V][O] **Defaults** (B1 instance ctor FUN_002af4f0 B1:239754-239759): SpawnTime 10, DecayTime 10, two fixed idle
  thresholds 10 s (+0x7c) and 20 s (+0x80), Team −1, armed with the countdown at 0 → the first spawn is immediate.
- [V][O] **Setter** B1 FUN_002af778: `ControlZone` → the CP by name (+0x74) and the pad takes the CP's team; `DecayTime`,
  `SpawnTime` (%f), `Team` (atoi) (B1:239839-239852). `Layer` has no branch. `ExpireTimeEnemy` / `ExpireTimeField` are in
  NEITHER ELF (§1) — the fixed 20 s / 10 s stand in for them.
- [V][O] **Which class for which team:** table DAT_005183a0, 7 rows × 3 hashes by the team's side (set by SetTeamName):
  Neutral, Alliance, Empire, Republic, CIS, Locals, Historical, each `Class<SideName>`, `Class<Side>Atk`,
  `Class<Side>Def` (re_check word 0x5183a0 ×21). Team t matches row[side][0] (any team) or, for t = 1, 2, row[side][t]
  (B1:239857-239875): **Atk = team 1, Def = team 2** of that side, not "who attacks this CP" [D]. Teams 1–2 are forced to
  the Historical row when mission+0xa4 is set (B1:239864; the writer [H]). BF2 DAT_005feeb0 is the same 21 words.
- [V][O] **Per tick** (B1 FUN_002b0780): the pad follows its CP's team; on a change the countdown resets to SpawnTime
  (B1:240799); if its vehicle lives → the decay check, else if a class exists for the team → spawn: arm SpawnTime once
  (B1:240445), count down, create the class at the pad (ground ray first: 1 m over the pad to 1 m under it, else the pad's own point —
  `re-vehicle-rest.md` §1) (B1:240521), retry in 1 s on failure. **One
  vehicle per pad in BF1**; BF2 adds `SpawnCount` (default 1) and keeps a list (B2:369466, 370750).
- [V][O] **Decay** (B1 FUN_002afa90): while nobody sits in it and no player is within r_player + r_vehicle + 10 m
  (B1:240254), an idle counter runs; past 20 s (CP lost or the vehicle outside the CP area, B1:240146) or 10 s (the CP's
  team now maps to a different class, B1:240222) it takes MaxHealth / DecayTime damage per second (B1:240338).
- [V][O] **Sabotage (BF1 only):** a new vehicle spawns with 15 % health (0.85·MaxHealth damage) when the enemy team's bonus
  bit 0x20 is set (B1:240640-240653); BF2's spawn has no such line.
- [V] **vehiclepad** (the same class with arg 1): a vehicle arrives with a carrier — itself if it passes a type test
  (DAT_005849f8, [H] flyer), else the team carrier from Lua `SetCarrierClass` (B1:239898-239907); the countdown starts at
  SpawnTime − the carrier's flight time (B1:240451). No stock map places a vehiclepad instance [V census].
- **VPAD build notes (2026-09-26, read while building; no verifier — not [O]):**
  - [V] The class table's rows (ELF words at 0x5183a0): `ClassNeutral` ×3; `ClassAlliance`/`ClassAllAtk`/`ClassAllDef`;
    `ClassEmpire`/`…Imp…`; `ClassRepublic`/`…Rep…`; `ClassCis`/`…Cis…`; `ClassLocals`/`…Loc…`; `ClassHistorical`/`…His…`.
    The setter walks teams 0..7 and stores the class for the FIRST team whose row matches (B1:239864-239875, store at
    +0x88 + 4·team B1:239890).
  - [V] `ControlZone` copies the post's team into +0xfc at load, only if the post entity is already bound
    (B1:239915-239968). Every stock pad authors `Team` before `ControlZone` (313/313, bake census); 21 come before their
    post in the manifest [H: whether the copy then happens].
  - [V] The ctor arms the pad (+0x100 = 1, countdown +0x104 = 0); a spawn clears +0x100, so the NEXT countdown loads
    SpawnTime (B1:240440-240447, 240668). A team change reloads +0x104 but not +0x100 (B1:240799-240800).
  - [V] The decay's thresholds: CP not alive (+0xac bit 3) or vehicle outside the post's ControlRegion (cp+0x70,
    FUN_002a06e8 → FUN_004c2b08; re-cp-capture.md: +0x70 = ControlRegion) → +0x80 (20 s); the class for the spawn-time
    team ≠ the current team's → +0x7c (10 s); else no decay and the idle count is NOT reset (B1:240146-240222). The idle
    count +0x108 resets only on a no-post / live-team / occupied check, never at a spawn [V: FUN_002b0238 does not write
    it]. Before those checks, a vehicle outside every region of the convex list DAT_00518314 (FUN_002956d8, half-plane
    test FUN_002958f8) skips the no-post / live-team reset — the list is unnamed [H].
  - [V] BF2's tick FUN_00373138 keeps a list: dead entries dropped, each live one decayed (FUN_00371ce0), then a spawn
    step while `count < SpawnCount` and a class (B2:370750). BF2's decay body is not opened [H: same as B1].
  - **2026-10-07 (card VPAD2): the BF2 pad is read in `re-vehicle-pads-load.md`** — fields, update, spawn step, the
    decay (the B1 rule; [V] there), and what this section lacked: a class name no LOADED class answers is a null slot
    (the port resolved against the whole side file: 11 BF2 slots differ), and both games tick every pad once with
    dt = 0 in the post-load step, BF2 before `ScriptPostLoad`. BF2's spawn step also ends in a 75 % health penalty
    behind a team-bonus test (B2:370533-370566) — "BF2's spawn has no such line" above is open again [H].

### 7.1 The vehicle's team — AUDIT 2026-09-25 [O]

Derived by the audit from the ELFs without reading §5–§8 first. Every line was checked with `re_check.py fn`, the
constants with `re_check.py word`, and the "only these writers" claims with `re_check.py absent` plus function-range
greps. One Opus verifier SUPPORTED all ten claims (A1–A10); its qualifications are folded in below.

**The live team.** The team is B1 entity+0xd8, and B2 the 4-bit field at +0x26c that `GetObjectTeam` reads (B2:34784).
This is the team that CanEnter, the team relation and targeting use. It follows three rules in both games:

- [V][O] **Any seat's new occupant sets the team.**
  - B1 FUN_0023c488, a seat's set-pilot, is non-virtual (0 data words, 39 direct calls). It fetches the owning entity
    through the seat vtable slot +0x18 (B1:154790) and calls SetTeam (vtable +0x8c) with the pilot's +0xd8
    (B1:154825-154829).
  - B2 FUN_002f5478 sits at seat-vtable slot +0x88 in 13 vtables, e.g. words 0x6200d8, 0x620398, 0x625c40. The only
    override is `remoteterminal`'s, which calls it first. It fetches the entity via +0x34 slot +0x24 (B2:283765) and calls
    SetTeam (+0x98, base FUN_00268f20 B2:185835) with the pilot's team (B2:283845-283846).
  - Every seat kind resolves to the VEHICLE (all words `re_check.py word`):

    | Seat kind | B1 getter | B2 getter |
    |---|---|---|
    | Own seat | 0x1c6b10 `addiu v0,a0,-0x2a0` | `addiu v0,a0,-0x288` |
    | MountedTurret | 0x2268d0 `lw v0,0x388(a0)` → the ctor's owner arg | 0x2711c8 `lw v0,0x250(a0)` |
    | PassengerSlot | 0x2283f8 `lw v0,0x2cc(a0)` | 0x2723e0 `lw v0,0x19c(a0)` |

    The constructors pass the vehicle as that owner: B1:83897, 83929; B2:114425, 114457, 130748, 130778.
  - So a gunner or a passenger alone sets the team, and the driver leaving does not revert it.
- [V][O] **The last occupant leaving → team 0 (neutral)**, when the class flag is set.
  - This happens when no seat in the ring has a pilot: B1 B1:154803-154818, B2 B2:283815-283836.
  - B1: B2 counts a PilotType-1 seat, and a type-4 seat piloting itself, as empty. B1 counts a self seat as occupied,
    because its seat ctor sets pilot = self.
  - The B1 flag is class+0x80. Default 1 in FUN_00221b28 (B1:135870). 0 in the `commandhover` / `commandwalker` class
    ctors (B1:224878, 230522). It is copied by the class copy ctor FUN_00221c40 (B1:135930). No write in the damageable,
    hover, flyer, walker or chunk setters.
  - The B2 flag is class+0x330 bit 34. Set in FUN_00269d98 (B2:186494). Cleared by `commandflyer` / `commandhover` /
    `commandwalker` (B2:348465, 348966, 354410) and by `mine` (B2:143657). Copied by FUN_00269f38 (B2:186591). No ODF
    property writes it.
- [V][O] **A pad gives an EMPTY ordinary vehicle no team.**
  - B1 FUN_002b0238 (B1:240605-240629) and B2 FUN_003725f0 (B2:370478-370520) call SetTeam with the pad's team only
    when:
    - the class flag is clear (command vehicles), or the class is null;
    - a seat is occupied (an AI pilot spawned with it);
    - or (B2) a seat is self-piloted.
  - The constructors write no team, and the GameObject ctor zeroes it (B1 FUN_00221200 B1:135451). So an empty
    ordinary vehicle stays team 0.
  - [V] The pad's team is ALSO recorded apart, and AI code reads it:
    - B1 records it once per CLASS at class+0x88, unless the ODF bool 0x0ffe3c06 set class+0x84 (B1:240631-240633,
      read in FUN_00241f18 B1:159173).
    - B2 records it in the vehicle's bits 40-43 unless class bit 35 is set (B2:370522-370526, read in FUN_002d6828 and
      FUN_002e2e38).
    - Their AI use is [H].
- [V][O] **Why it matters: CanEnter** (B1 FUN_0020b1b0).
  - A vehicle with team < 1 passes the team test for anyone (B1:120029).
  - Otherwise an AI needs the same team (B1:120032-120034) and a player needs a friendly relation (FUN_00221890 →
    FUN_00221788, B1:120039).
  - So an empty vehicle is open to both sides. An occupied one, or a command vehicle, is closed to the enemy.
- [D] **For the port.**
  - `team` = the live team: `None` = PS2 team 0. Every seat's entrant sets it. It goes to `None` when the last rider
    leaves, except for command* classes.
  - The pad's team is a separate `home_team`.
  - VENT's `vehicle_team` (driver-seat team, else the pad's) is the defect. Card VTEAM.

## 8. Turrets on buildings (Q5)

> **AUDIT 2026-09-29 (VTURB landing f279070 + 6ea549d): PARTLY** (Opus verifier 12/12 SUPPORTED;
> `engine/artifacts/loop/reports/20260929_0545_audit_VTURB.md`, `engine/artifacts/lead_tmp/audit-vturb/`).
>
> **Held:**
> - The section split (B1 FUN_001c4db8 B1:70347, B2 twin FUN_001e8948 B2:97129). Other lines reach the MountedTurret
>   setter first, then the building's.
> - No seat or PassengerSlots of its own: FUN_0023d778 is called only from the vehicle ctors (+0x14b0) and the
>   MountedTurret ctor (+0x50).
> - HealthType defaults to 4 (FUN_001c08b0 B1:67215). BF1's PilotType defaults to 2 (FUN_0023d778 B1:155813).
> - Class lookup folds case (FUN_001769d8 → FUN_00444b30).
> - The port spawns exactly the disc's instances on all 15 baked BF1 maps.
>
> **Corrections:**
> - A TURRET section's `YawLimits` / `PitchLimits` are the MountedTurret's (+0x1124/+0x1128, +0x112c/+0x1130, ctor ±10
>   rad), not the aimer's (§9's ±30° / ±45° are `AimerYawLimits` / `AimerPitchLimits`). The tick FUN_00223850
>   (B1:137499-137517) clamps yaw only for a wrap-flag range, or when min > -3.1416926 and max < 3.1414928. Otherwise it
>   wraps, so unauthored and "-180 180" limits turn continuously.
> - The exit ray ignores the seat's entity, which for a building turret is the building (FUN_002268d0 → turret+0x398;
>   FUN_0020b918 → FUN_00161cb8(…, list, 1, 0x4c) → FUN_00162b60).
> - A barrel with no `FirePointName` takes its `BarrelNodeName` as the fire point (FUN_0026e260 B1:192569 / 192606).
> - `portableturret` / `defensegridturret` are separate engine labels in the B2 ELF, and the ODFs name them as BASE,
>   not `armedbuildingdynamic`.
> - A dead armed building heals back over `RespawnTime` (default 120 s, FUN_001c4cd0 B1:70305; FUN_001bfb88
>   B1:66848-66853).
>
> Cards VTURA, VTURD, VTUR2.

- [V][O] BF1 `armedbuilding` (com_weap_gunturret) uses the SAME TURRET/TURRET1..8 table, the same 0x11b0 MountedTurret
  section and the same section setter FUN_00225f08 (B1 FUN_001c4db8 B1:70353); BF2 `armedbuilding` FUN_001e8860 → setter
  FUN_001e8948 → FUN_00270770, and `portableturret` / `defensegridturret` are `armedbuildingdynamic` subclasses of it.
- [V][O] An armed building has no seat of its own (neither Controllable setter's callers include it), so its only seats
  are its MountedTurret sections (kind 7) — each turret object does carry seat props, since the section setter falls
  through to the Controllable setter (B1:139045); the building itself is kind 8, which the enter search treats as
  "turret" (B1:119710).
- **Scope [D]:** yes — the port can model a turret as a vehicle with no movement, no pilot seat and 1..8 gunner seats,
  sharing the section props, seat props and the enter/exit/placement code. [H]: the armed building's entity init (seat
  ring, +0xac bit 3) was not opened.

## 9. Aiming in and against vehicles (Q6)

- [V][O] **BF1 magnet vs vehicles.** The player's magnet target is acquired in the human-controller update B1
  FUN_0025e098: a sphere query of radius weaponClass+0xd0 (≤ 300 hits, mask 0x1e, B1:180671), each candidate filtered by
  the weapon's team bits (+0xf4, via FUN_00221788) and target-kind bits (+0xf8, by the candidate's kind `flags & 7`)
  (B1:180679-180694), scored by the same cone function as vehicle entry, an LOS ray (mask 0x48); the non-null pick is
  stored at weapon+0x128 only when it passed the filter and lies within LockOnRange + its radius (B1:180603-180621,
  180870, 180882-180883); otherwise weapon+0x128 is cleared (B1:180891-180892). A vehicle is kind 3 (§6);
  `TargetVehicle` sets +0xf8 bit 3 (B1:209997) and the weapon-class ctor defaults +0xf8 to all kinds and +0xf4 to
  enemies (B1:209490, 209497) → **by default any weapon with LockOnRange > 0 magnets onto enemy vehicles** [D].
  *Qualified by AIMF (2026-09-28, `re-aim-assist.md` §3.8):* "by default" means only weapons that OMIT
  TargetVehicle. Every BF1 infantry blaster (rifle, pistol, shotgun, bowcaster, repeater, wrist weapons) writes
  TargetVehicle 0 and never magnets onto a vehicle. The launchers, crack grenades and vehicle cannons that write 1 or
  omit it do. The acquisition itself is transcribed in `re-aim-assist.md` §3.
- [V][D] **In a vehicle:** the magnetism core FUN_0025fea8 (called B1:179737) has no soldier/vehicle test (its bail-outs:
  remote index, no weapon, no target, B1:180977-180985), so it runs for a driver and a gunner whenever the seat's weapon
  has LockOnRange > 0 [D]. BF1 data: AT-ST chin 100 / head 120, AT-AT 90 / 150, AT-TE 100, speeder bike 100, snowspeeder
  100, hovernaut 200, fighters 80, gunship 150–250; none (0) on the fighter tank, AAT, STAP, combat speeder, spider
  walker and the building gun turret. No AimFactor/AimAssist hash exists in either ELF.
- **Correction to `analysis/re-aim-assist.md` §3 [V][O]:** its acquisition function B1 FUN_00223850 is the
  **MountedTurret's** tick, not the soldier's: its only caller is the thunk FUN_00226860 (B1:139310), whose one data
  reference is the vtable slot 0x5342fc; that vtable's typeinfo function FUN_00226ed0 references `13MountedTurret`
  (0x534578). The soldier's tick is FUN_00205cc8 (vtable 0x532bc0). So §3's "the player path applies no team filter" is
  wrong: the soldier's acquisition (above) filters by team and kind and uses a cone, not a ray. The port copies §3:
  `core::sim::input` `Sim::acquire_magnet` takes the nearest capsule the aim ray hits, any team
  (`engine/crates/core/src/sim/input.rs:118-145`) — card AIMF.
  *Qualified by VTAT (2026-10-07, `re-turret-ai.md` 5):* that ray is not "the MountedTurret's acquisition" in general
  either. It runs only for a seat no human drives whose CLASS says `Controllable = 0` (K+0x1150, default 1) — the
  Naboo fambaa's two classes; an AI gunner's target comes from its UnitStationaryController's agent
  (`re-turret-ai.md` 2-4), and a `PilotType = self` seat is BF2's automatic turret (section 3 there).
- *(Corrected 2026-10-22, card UORD, `re-ordnance-kinds.md` §4-§5: the missile's steering is read and measured
  there; BF1's lock square is B1:262320-262339 (B1:262211 below is the refire alpha); BF2's Controllable ticks copy
  TargetLockMaxDistance / ...Lose and the Target* mask, not LockOnRange / LockOnAngle / LockOffAngle.)*
- [V] **Lock-on.** BF1 has a dwell lock on launchers only: `LockTime` is parsed only by the launcher class (B1:217009);
  the lock timer counts while weapon+0x128 holds the same target and locks at LockTime (B1:216739-216794); the HUD draws
  the progress (B1:262211); a missile copies the shooter's target at spawn (B1:202829-202834; its steering [H]). BF2: the
  weapon class parses LockOnRange/LockOnAngle/LockOffAngle and `TargetLockMaxDistance` / `…Lose` (B2 FUN_00328fb0,
  B2:319976-320750), and six Controllable ticks — droideka, flyer, hover, soldier, walker, MountedTurret by module [D] —
  copy them into one per-player lock object (B2:109086, 116820, 132164, 159266, 179562, 188918): **one lock-on mechanism
  on foot and in every vehicle kind** [D]. The lock button and BF2's acquisition loop were not located [H]. BF2's
  `com_weap_veh_guided_rocket_ord` is a `flyer` entity fired by a `remote` weapon: a player-steered rocket, not homing
  ordnance [D].
- [V] **Turret aim:** an aimer (B1 FUN_0026e260; defaults pitch ±45°, yaw ±30°, PitchRate 1.0, YawRate 2.0 rad/s,
  B1:192359-192366) turns toward a world point at ≤ rate·dt per axis (B1:191984-192000), clamped to its limits
  (B1:192137); vehicle seats cast a 512 m ray along the camera aim (hover B1:98294-98302, MountedTurret B1:138065) and
  aim at its hit (or at the magnet target's aim point). BF2 FUN_00302f10 has the same hashes at shifted offsets.
  *Qualified by the VTURB audit (2026-09-29, §8 banner):*
  - These defaults and limits are the aimer's (`AimerYawLimits` / `AimerPitchLimits`).
  - A section's `YawLimits` / `PitchLimits` bound the MountedTurret itself, whose tick wraps unless the range passes
    FUN_00223850's test.

## 10. What the port already has, and what M2 must add (engine survey, main 9aaf5eb)

Already there (reuse, do not rebuild):
- **Input:** `core::sim::input::buttons::ACTION` ("enter vehicle / interact", bit 6) and N pads indexed by player slot
  (P9(a), done) — seat routing can be slot-based from the first line, as PLAN's P7 design requires.
- **Damage table:** `core::weapon::HealthType::Vehicle` and each ordnance's `VehicleScale` are parsed and tested
  (`weapon.rs` — "Vehicles are immune to the BF1 rifle (VehicleScale 0)"); a vehicle's hull health can take bolts through
  the existing table once `Hit::Target` (VPRE) exists.
- **Collision:** `core::collision::Layers::VEHICLE` (bit 4) is RESERVED and "never set by anything today"; the bake
  already emits every class's `VehicleCollision` / `VehicleCollisionOnly` bodies (`ManifestClass.collision`,
  `game::collision_layers`, analysis/re-collision-layers.md). `CollisionWorld::move_body` (DR2's doors) is the precedent
  for a body that moves.
- **Classes:** `game::classdb::ClassDb` resolves side + common classes with their BASE chain
  (`assets_baked/classes/{bf1,bf2}_*.ron`), so every SIDE vehicle's props are already baked (hashes → values).
- **Pads:** every `com_item_vehicle_spawn` instance and its props are in each map's `manifest.ron` (`ManifestInstance`).
- **Models:** map-local vehicle models are baked as map models: TAT1's `all_hover_skiff` (+ LOWD, `skiff_chunk1..3`, its
  collision GLB) and BF1 BES1's cloud car.
- **AI:** the PS2's vehicle-aware navigation constants are already [V] in RE_LOG (arrival radius EntityHover =
  class+0x1788 ForwardSpeed × 0.5, others 4.0 m; look-ahead |velocity| for hovers, 20 m for walkers; flyers 3-D;
  AISizeType masks HOVER/SMALL/MEDIUM/HUGE/FLYER over the planning graph).

Missing (each becomes part of a card in §11):
1. A vehicle entity (VENT): `Sim.vehicles`, handles, seats, `Unit.mount`, the hash append.
2. Map-local class PROPS: a map bake keeps only a class's geometry and collision (`ManifestClass`), so the skiff's and
   cloud car's drive/seat props are not baked — the map bake must also write its `entc` props (the `bake classes` code
   already parses any LVL).
3. SIDE vehicle models: the side bake writes soldiers and weapons only (`assets_baked/side_bf1/{soldiers,weapons}`); no
   speeder bike, AT-ST or X-wing model is baked. Skinned vehicles (walkers) hit `bake::soldier::segment_blends`' refusal
   of the 0x02 bit and of a 0x04 word with 0x08/0x40/0x80 (`rep_walk_atte` carries 0x02 — PLAN HOLD 1 note).
   **Done by VBAKE (2026-09-26):** `bake vehicle` (`bake/src/vehicle.rs`) writes every vehicle-BASE class of the
   four sides of both games (75 classes) to `side_<game>/vehicles/<side>/` — every LOD, the chunk models, `coll` +
   `prim`, the layer lists. Every model is RIGID in its `skel` bind pose through the world path (ALPHA RULE v2 draws
   the 0x02 / 0x85 / 0x10089 words the skinned path refuses); the skinned walker pass stays VWLK's. [V] census: every
   side vehicle model has a same-named `skel`; most are bone-space (bikes, tanks, walkers, most fighters). 13 chunk /
   LOD models name a texture only ANOTHER side LVL ships (BF1 IMP fighter-tank chunks → `rep_hover_fightertank`, BF1
   REP speeder-bike chunks → `imp_hover_speederbike`, BF1 REP `rep_fly_vtrans_01`, BF2 ALL gunship LOWD/hulk →
   `rep_la_at_*`) — the runtime's texture lookup must span every loaded LVL, as the PS2's does [H: what the PS2 draws
   when the other side is not loaded].
4. The world's vehicle collision subset (set `Layers::VEHICLE` from the baked layer lists) and vehicle hulls in
   `CollisionWorld` for soldier push-out and bolts.
5. A vehicle camera (EyePointOffset / TrackCenter / TrackOffset / TiltValue / ThirdPersonFOV per seat), HUD, sounds.

## 11. The M2 build plan — cards (created 2026-09-25 by VEH0)

Order to the first drivable vehicle (TAT1's skiff: its model, chunks and collision are already in the TAT1 bake, its
props only need VCLS; the pads place it for every era): **VPRE ∥ HVRE ∥ VCLS → VENT → HOVR → VSEAT**. Then pads, damage,
turrets, AI, the side models, walkers, flyers.

| card | phase | value | deps | what |
|---|---|---|---|---|
| VPRE | cleanup | 5 | — | `CollisionWorld.bodies` private + `Hit::Target(Target::Unit)` — landed da4ecc4 while VEH0 ran |
| VENT | build | 5 | — | (exists, amended) `Sim.vehicles`, handles, the seat ring (pilot / ≤ 8 gunners / ≤ 4 passengers), ONE hull health, `Unit.mount`, the hash append |
| HVRE | re | 5 | — | transcribe BF2's hover tick / spring / drive / contact / position step (§3) into `analysis/re-hover.md` |
| VCLS | build | 5 | — | bake each map's own `entc` props (skiff, cloud car); typed hover / seat class readers with the §3.1 defaults |
| HOVR | build | 5 | VENT, HVRE, VCLS | `core::vehicle::hover` — the BF2 integrator on every hover/treaded class; the skiff on a scripted pad |
| VSEAT | build | 5 | HOVR | enter / drive / exit on TAT1 (§5 range, seat order, placement), the vehicle camera, the skiff drawn — **the first drivable vehicle** |
| VPAD | build | 4 | VENT, VCLS | spawn pads (§7): class per team side, CP follow, SpawnTime, one per pad / SpawnCount, decay |
| VBAKE | build | 4 | — | side vehicle models, chunks and collision from the SIDE LVLs (hover/flyer rigid first; the skinned walkers' blend-bit pass) |
| VDMG | build | 4 | VENT, VPRE | hull health via VehicleScale, collision damage, damage-effect bands, death = explosion + chunks, no wreck |
| VTUR | build | 4 | VSEAT | gunner seats and vehicle weapons (aimers §9, fire points), building turrets as seat-only vehicles (§8), the in-vehicle aim decision |
| VAI | build | 4 | VSEAT, VPAD | AI boards (§5), drives with RE_LOG's vehicle nav constants, fires, dismounts |
| VWLK | re | 3 | — | the walker: BF2 FUN_0025d5e0 root-motion locomotion + legs → build card |
| VFLY | re | 3 | — | the flyer: the 5-state integrator (BF1 flyers, faithful per map) → build card |
| AIMF | re | 4 (M3) | — | the SOLDIER's magnet acquisition is FUN_0025e098's filtered cone, not the MountedTurret ray the port copies (§9) |

Design deltas for PLAN's P7 entity shape (VENT applies them): `health` is ONE pool, not `Vec<RegionHealth>` (§6), so
`Target::Vehicle(handle)` needs no region; `Seat.kind` = Pilot | Gunner(section 0..7) | Passenger, in the PS2's ring order
(§5); ~~the team comes from the pad (§7)~~ **AUDIT 2026-09-25: the live team is team 0 when empty, the last entrant's in
any seat, command vehicles never neutral; the pad's team is a separate home team (§7.1, card VTEAM)**; no respawn field on
the vehicle (the pad owns respawn).

## 12. Checks run, the verifier, what stayed [H]

- Check 1 (`re_check.py fn`): every line cited here was checked by the lead or a helper (helpers' appendices list them;
  the lead re-ran fn on each helper line it reproduced: FLYERSECTION table B1:102492-102546, enter cone
  B1:119520/119544/158895, exit rays B1:120506-120624, PilotType B1:156006-156035, death B1:97306/97314, pad
  B1:240146-240653, walker B1:129697-129882 and B2:178080-178536, flyer B1:85679-85830, aim B1:180600-180890).
- Check 2 (absence): ExpireTimeEnemy/Field (elfword + re_check absent, both ELFs), BF2 strafe fold (no `ABS(` in
  FUN_0021b470), DestroyedGeometryName on vehicles (hashloc), AimFactor/AimAssist hashes (hashloc 0), no vehiclepad
  instance and no directly placed vehicle (manifest scans).
- Check 3 (twins): every [O] mechanism has its twin located in the other ELF (by the lead or a helper) except the hover
  contact callback (B2 twin not opened), the aimer runtime (B2 assumed) and the damage-band reader (found in neither).
- Check 4 (bytes): hover literals (−9.8 / 9.8 lui+ori, 10.0 / 9.0 lone lui), DAT_00517810 = 0.1, the pad class table
  0x5183a0 / 0x5feeb0 (21 words), the 0.7071 exit literal, the BF2 flyer caps 0x5fcf24.., vtable words 0x532c28 /
  0x5342f0.
- Check 5 (defaults and writers): DAT_00517814 (section index) writers; DAT_00517824 (passenger mode) writers; the fly band
  DAT_005873b4/b8 writers; hover ctor defaults both games; NoRandomSpring's default (B2 +0xe90 = 1).
- **Verifier** (one Opus agent, foreground, given `lead_tmp/veh0/verify/claims.md` only — 42 claims L1-L17, S1-S8, D1-D7,
  W1-W5, A1-A5; verdicts `lead_tmp/veh0/verify/VERDICTS.md`): **38 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED.** Fixed here:
  L11 (the yaw-rate formula was only the AI's; the human branches are now in §3.2 item 7), L13 (the boost flag is the byte
  at entity+0x1ddc, a human also needs the bit-0 input or the flag, and an AI can boost — §3.3), W2 (the two BF2 walker
  clock rules are separate branches, not an if/else — §4.1), A2 (weapon+0x128 is also cleared in the else branch — §9).
  Its caveats applied: the lift's SetAltitude gate, the horizontal-only rebound and the other body's damage factor
  (§3.2), the death path's net-flag nuance (§6), the pad-default and exit-ray line numbers (§5, §7), and that turret
  objects carry seat props through the section setter's fall-through (§8). Every absence claim survived its own
  falsification search (ExpireTime* incl. negated and decimal forms and strings; DestroyedGeometryName through the
  vehicle setters' base chain).
- **Stayed [H]:** the hover position step and B2 contact twin (HVRE); where DamageStart/Stop bands are read; occupants
  killed or ejected at death; chunk lifetime; the recharge station's and repair weapon's runtime; the historical-mode
  writer; the physical enter button; the armed building's seat ring; BF2's lock button and acquisition loop; the walker
  control call's float args; flyer state 4 and the B2 flyer mode flag +0x668's writer; ~100 unnamed hashes (none on a
  movement path this doc relies on).
