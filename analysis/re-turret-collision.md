# OPUS RE (loop lane 1, 2026-10-21, card TCOL) - [O] UNAUDITED
# RE: a building turret as a body - where its collision sits and moves, who a hit on it hurts, its gunner, its death

**The questions** (card TCOL; the maintainer's INBOX 2026-10-21 18:40 item 2, BF2 Geonosis). Rockets, bolts and grenades pass
through the UPPER half of a Geonosis turret cannon and hit its base; its health never drops; its gunner takes no damage.
(1) Where does the PS2 put a building turret's collision prims, and do they move with the gun? (2) Does a hit on the
turret hurt the building, the gunner, or both; does the gunner take blasts; can a bolt hit him? (3) Can a turret's
health reach 0, and what then? (4) Census: every armed-building class on both discs, its bodies against the PS2's.

**Method.** The body-matrix function and its callers read in both decomps, the turret / aimer / barrel registration
read at the ELF bytes; a disc census of every armed-building model's `prim` / `coll` / `skel` chunks; RAM: the
collision world's lists in BF2 and BF1 save states; two live BF2 Hoth bots battles of 240 game-seconds each, one row
per game update of every armed building's health and its seated gunner's health (`gunner_live.py`). Tags: [V] read at
the cited lines (B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c`) or ELF bytes; [L] measured on the PS2's
RAM or disc; [D] derived; [H] hypothesis. E = an entity, K = its class, T = a MountedTurret, P = a prim.

## 0. Short answers

1. **A building turret's prims on the turret, aimer or barrel node turn with the gun** [O][V]. Every collision body's
   model-space matrix is its XFRM through the matrix its PRNT names in the owner's pose table (both games,
   `FUN_00160160`), and an armed building hands its turrets the SAME table its collision queries read; the turret
   node, the aimer node and each barrel node are entered there as pointers to their live, per-update matrices. So a
   prim under `aimer_x` follows the gun's yaw and pitch; a prim under any other node rests at that node's bind; the
   `coll` mesh (parent 0) is at the model root.
2. **The port's identity rule is exactly the bug** [O][L]. Of the 47 placed armed-building classes, 0 prims on a
   static node have a non-identity bind; the gap is only the posed-node prims. Geonosis's `geo_bldg_geoturret` (both
   games): 3 of its 4 prims hang on `aimer_x` (bind 1.60 m up): PS2 collision top 3.18 m, drawn top 3.21 m; at the
   identity the gun cylinders sink into the base and the top is 1.58 m - nothing from 1.6 to 3.2 m.
3. **A hit on the building damages the building only** [O][L]: 38 game updates in which a manned turret's building
   lost 7 - 1,600 health (cannon, launcher, grenade, rifle-scale hits) left its seated gunner's health unchanged.
4. **The gunner is a body of his own when he is drawn** [O][L][V]: a drawn gunner (a pose key) stays in the collision
   world at his seat with the riders' type (BF2 0x20, 7 of 7 dish gunners 1.35 - 1.59 m from the building origin;
   BF1 0x10, 84 of 84 soldiers in EntityBuildingArmed seats); a hidden gunner has no collision object (3 of 3,
   at the building origin). Every flying ordnance class's per-update ray tests the riders' type (BF2 masks
   0x13be / 0x132e, BF1 0x9de / 0x996), so a bolt or rocket can hit a drawn gunner directly; a blast hurts a gunner
   mounted on a building (the only mount whose rider takes splash), past its LOS test.
5. **Health 0 destroys the turret, kills the gunner, and the turret can come back** [O][V][L]: the death switches each
   turret off and kills its occupant with the building's last-damage record (the vehicle-rider kill, both games);
   live 8 of 8 manned deaths killed the gunner in the same update (4 drawn gunners left corpses, 4 hidden ones were
   deleted). The building then takes its destroyed model AND its destroyed collision (state 1), or its pad and the
   pad's collision (state 2), and regenerates MaxHealth / RespawnTime (`re-build-zones.md` 1). Live: 10 of 24
   BF2 Hoth turrets fell in 240 s and two were back at full health (state 0) inside the run.

## 1. The collision bodies of an armed building [O][V]

1.1 **The body's matrix.** BF2 `FUN_00160160` (B2:121-218): with a pose table (argument 3 != 0) and a non-zero parent
    hash at body+0x48 (MIPS 0x160180 `lw a2, 72(s0)` - the hash Ghidra drops from the call), the matrix is
    `… with no
    table, a zero hash or a failed lookup, the XFRM alone (B2:141-145, 192-217). `FUN_00160258` (B2:221-283) then
    applies the entity's world matrix (B2:241). BF1 `FUN_00160160` (B1:121-220) is the same, the hash at body+0x44
    (MIPS 0x160180 `lw a2, 68(s0)`), lookup `FUN_00444998` (B1:147).
1.2 **Which table an entity hands the queries.** The ray: the grid ray's per-object callback `FUN_00162f60`
    (B2:2542-2588) calls slot +0x24 of the object's vtable; for an armed building (E+0x10, vtable 0x61a410) that is
    `FUN_001691a0` (word 0x61a434; B2:7732-7755): with a model (obj+0x60 = E+0x70) it takes the matrix from slot +0x48
    and the table from slot +0x4c and calls `FUN_00168318` (B2:6850-; model, matrix, table in a0-a2), which moves the
    ray into the model's frame and hands that table to every body's ray slot +0x18 (B2:6923-6924; the bodies of the
    query's subset only); a prim's (vtable 0x60ef58, word 0x60ef70) is `FUN_00160518` (B2:371-381): `FUN_00160160(tmp,
    body, table)`, then the shape's ray. The target-point queries do the same: BF2 `FUN_002133c8` / `FUN_00213488`
    (B2:126720-126825; model E+0x70, matrix E+0x120, table from slot +0x4c on E+0x10, B2:126740, 126783, 126812 - the
    first two only when the model has bodies and a Targetable subset); BF1 `FUN_001e2460` / `FUN_001e2500`
    (B1:91606-91660): model E+0x160, matrix E+0x200, the table from slot +0x44 of the vtable at E+0x1c0 on E+0x100
    (B1:91621-91622, 91653-91654).
1.3 **The armed building's table.** BF2 ctor `FUN_001e7a30` writes E+0xcc = 0x61a410 (B2:96402); its slot +0x4c
    (word 0x61a45c = 0x1e8db8: `jr ra; addiu v0, a0, 0x298`) returns (E+0x10)+0x298 = **E+0x2a8**; slot +0x48 (0x61a458
    = `FUN_001e52c8`, B2:94744-94750) returns E+0x120. BF1 ctor `FUN_001c4378` writes E+0x1c0 = 0x52eec8 (B1:69830);
    slot +0x44 (word 0x52ef0c = 0x1c5328: `addiu v0, a0, 0x1d8`) returns (E+0x100)+0x1d8 = **E+0x2d8**; slot +0x40
    (0x52ef08 = 0x1c1088) returns E+0x200.
1.4 **The turrets share it.** The same ctor builds each turret with that table - BF2 `FUN_0026bf88(…, E+0x2a8)`
    (B2:96420), stored at T+0x280 (B2:187898), the owner at T+0x278 (B2:187911); BF1 `FUN_00222f20(…, E+0x2d8)`
    (B1:69844) - and then fills it with a pointer to the skeleton's bind matrix for every node not yet entered (BF2
    `FUN_0057d6c0` B2:96439 -> `FUN_0057f088` B2:714470-714510, an insert only when the lookup fails; BF1
    `FUN_004969d0` B1:69863 -> `FUN_00498ad8` B1:559330-559366).
1.5 **The live entries.** BF2 turret tick `FUN_0026cea0`: the turret node's matrix T+0x200 is rewritten every update
    (B2:189478-189493) and its table entry made a pointer to it (look up K+0x818's hash in T+0x280+8; when it is not
    T+0x200, remove and insert T+0x200; B2:189498-189517). The aimer node and every barrel node: `FUN_003018d8`
    (B2:293246; its table is a2), the same find / remove / insert (MIPS 0x301ab8-0x301b04 and 0x301c28-0x301c90:
    `jal 0x539150`, `0x539220`, `0x5391b0`; the parents are looked up at 0x301930 / 0x301b68), from the aimer's update
    and the owner's draw, which passes its table (`FUN_0026f610` B2:190702, a2 = its argument 4; `re-turret-draw.md` 2.3).
    BF1 `FUN_0026ce10` writes the aimer node into the entry the table holds for it (B1:191242-191246); the turret node
    T+0x320 likewise (`re-turret-draw.md` 2.2, B1:137537-137635).
1.6 [D] So for an armed building: P on the turret node -> P.XFRM x the turret node's live matrix (yaw only for
    every seated class but BF1 Bespin's AA gun, PitchTurnFactor 0.0); P on the aimer or a barrel node -> the gun's
    whole direction; P on any other node -> that node's bind; the mesh -> the model root.
1.7 **The mesh.** BF2 `FUN_001e4970` builds `CollisionMesh` / `DestroyedCollisionMesh` / `UnbuiltCollisionMesh`
    bodies (strings 0x619898 / 0x6198a8 / 0x6198c8) with the identity XFRM 0x73d8c0 and parent hash 0 (B2:94422,
    94448, 94483) into K+0x168 / K+0x388 / K+0x4d8; `GeometryName`'s prims `%s%d` go into K+0x168 (`FUN_00214600`
    B2:127748-127760), `DestroyedGeometryName`'s into K+0x388 (B2:94452-94459). BF1: `FUN_001c0b10`, identity
    0x66ef20, parent 0 (`re-collision-layers.md` 2).

## 2. The census [O][L] (`lead_tmp/tcol/primcensus.py`, raw LVL chunks; drawn tops from the baked GLBs)

Every placed armed-building class, both discs: `census_table.md` (BF1 7, BF2 40 classes; the per-prim rows in
`census_bf1.txt` / `census_bf2.txt`). A cylinder's DATA b is its FULL height (the watchtower's 15.41 m cylinder at
y 7.64 tops out at 15.34, its mesh box's top). Findings:
- The prims on a static node: every one has an identity bind (0 of them move between the identity and the bind).
- The ground turrets the port fields (soldier seats, 6 maps and more):

| class (game) | model | prims | on a posed node | collision top PS2 / identity / drawn top |
|---|---|---|---|---|
| geo_bldg_geoturret (B1), tur_bldg_geoturret (B2) | geo_bldg_geoturret | 4 | 3 (aimer_x) | 3.18 / 1.58 / 3.21 |
| bes_bldg_aaa_turret (B1) | kam_prop_antiaircraft | 21 | 20 | 4.46 / 2.17 / 4.49 |
| tur_bldg_recoilless_lg / _myg_auto / _kas (B2) | kam_prop_antiaircraft | 24 | 20 | 5.04 / 5.04 / 4.49 |
| tur_bldg_recoilless_fel_auto / _nab_auto (B2) | fel1_weap_prop_turret | 24 | 23 | 5.04 / 5.04 / 5.04 |
| hoth_bldg_dish_turret (B1) | hoth_bldg_dish_turret | 1 | 1 (turret_x) | 3.28 / 2.36 / 3.35 |
| tur_bldg_hoth_dishturret (B2) | hoth_bldg_dish_turret | 6 | 5 | 3.40 / 3.40 / 3.35 |
| hoth_bldg_hoth_turret (B1), tur_bldg_hoth_lasermortar (B2) | hoth_bldg_hoth_turret | 4 | 2 | 3.93 / 3.58 / 4.14 |
| com_weap_gunturret (B1), tur_bldg_laser / _beam / _mortar / tur_weap_built_gunturret (B2) | com_weap_gunturret | 14 | 12 | 3.89 / 3.89 / 1.91 |
| tur_bldg_chaingun_roof (B2) | com_bldg_chaingun_roof | 2 | 2 | 2.36 / 0.24 / 2.37 |
| yav_bldg_tower_turret (B1), tur_bldg_tower (B2) | yav_bldg_watchtower | 1 | 0 | 15.35 / 15.35 / 15.75 |
| tat_bldg_barge_turret (B1), tur_bldg_tat_barge (B2) | tat_bldg_barge_turret | 0 | 0 (mesh only) | - |

  Where the two tops agree (the BF2 Mygeeto / Felucia guns, the BF2 dish) the model binds the moving node at the
  identity: the bodies are right at rest and wrong as soon as the gun turns. Where they differ (the Geonosis cannon,
  the BF1 Bespin AA gun, the BF1 dish, the roof chaingun) the gun's bodies are missing from the gun even at rest.
  The space-map classes (no port map) are in the table for completeness.

## 3. Hits, blasts and the gunner [O]

3.1 **Damage to the building stays in the building** [L] (`live/hot1_ana.txt`, `gunner_ana.py`; BF2 HOT1 bots
    battle, 240 s, 5,405 rows): 38 updates with a manned building's health falling (7 - 1,600; weapons by the
    last-hit instance's vtable: 13 + 8 + 4 + 3 + 2 WeaponCannon, 5 + 1 WeaponGrenade, 2 WeaponLauncher) and its
    seated gunner unchanged; 3 updates both fell, by different amounts (62.2 / 48.2, 37.1 / 16.5 cannon; 450 / 450
    grenade) - splash; 2 updates the gunner alone (10.3, 4.9; a cannon). The building's health is E+0x170 / max
    E+0x174 (`re-build-zones.md` 1).
3.2 **Where the gunner's body is** [L-RAM] (`lists_b2.py`, `lead_tmp/vturd/ps2/hot1_bots_battle.p2s` and
    `live/hot1_end.p2s`): the seated soldier is T+0xcc - 0x270 (the seat's Controllable is E+0x270, as the rider kill
    reads it, B2:283473). 7 of 7 dish-turret gunners (a pose key: drawn) are on list 0x20 (+0x7c) with collision type
    0x20, 1.35 - 1.59 m from the building origin and 0.15 m up, their sphere (r 1.46) centred 1.0 m above the feet;
    3 of 3 laser-tower gunners (no pose key: hidden) are on no list, at the origin (`re-turret-draw.md` 3.1).
    BF1 (`seats_b1.py`, 153 states): 84 of 84 soldiers seated in an EntityBuildingArmed turret (seat = T+0x10) are
    on list 0x10 with type 0x10, state 12 (as RIDK's vehicle riders).
3.3 **Ordnance tests that type every update** [V]. The broadphase walks lists by bit (BF2 `FUN_001627d0`
    B2:2152-2178: 1 +0x18, 2 +0x2c, 8 +0x40, 4 +0x54, 0x10 +0x68, 0x20 +0x7c, 0x200 +0x90, 0x1000 +0xa4) and the
    grid tests `mask & type` per object (`FUN_001b3278` B2:58705). The per-update collide of every flying ordnance -
    BF2 `FUN_0030fad0`, slot +0x38 of the Ordnance, OrdnanceBeam, OrdnanceBullet, OrdnanceGrapplingHook,
    OrdnanceLaser (0x627308), OrdnanceMeleeThrow, OrdnanceMissile, OrdnanceShell and OrdnanceTowCable vtables, run
    for each ordnance by `FUN_00163770` (B2:2881-2902) - rays with mask 0x13be, or 0x132e once the static ray has run
    (B2:301923-301927; MIPS 0x30fbb4 / 0x30fbbc); both hold 2 (soldiers) and 0x20 (riders). BF1 `FUN_00273128`
    (B1:195436-195514; in the same eight classes' vtables) uses 0x9de / 0x996 (MIPS 0x27320c / 0x273214), both
    holding 2 and BF1's rider type 0x10. The ODF base `bolt` builds the laser class (`FUN_0031ba10`, the same as
    `laser`: B2:16169-16170, 16196-16197).
    [D] A bolt, bullet, missile or shell can hit a drawn gunner's own body; a hidden one has none to hit. Not seen
    live: which of 3.1's two gunner-only falls was a direct hit (the cannon also does 1,000 to buildings).
3.4 **Blasts** [V]: the soldier's splash returns without damage when he is mounted and the mount's collision type is
    not the building type (BF2 `FUN_00239480` B2:152650-152664, `!= 0x10`; BF1 `FUN_00200e68` B1:112808-112822,
    `!= 8`) - so a building turret's gunner takes splash, a vehicle's rider does not (`re-vehicle-death.md` 5). The
    blast's LOS ignores the victim itself and is blocked by buildings and terrain only (`re-vehicle-death.md` 2,
    B1:194099, B2:300680): a gunner behind his own building's bodies is shielded by them (3.1: grenade falls of 450
    on a manned dish once with and once without its gunner).

## 4. The turret's death [O]

4.1 **The gunner is killed** [V]. BF2 armed-building death `FUN_001e8498` (B2:96870-96925): per turret, slot +0x1c of
    the vtable at T+0x24 (0x620050; word 0x62006c = `FUN_00271268` -> `FUN_0026c6d0`, B2:191866) and the task
    removal `FUN_0026f470` (B2:96916-96917); `FUN_0026c6d0` (B2:188218-188234) stops the turret's sounds and calls
    `FUN_002f4e00(T+0x10)` (B2:188232), the rider kill: for a soldier pilot (type 2; type 4 counts as 2 unless the
    pilot is the Controllable itself), with the cheat flags clear and the owner non-zero, the pilot's health slot +0x20
    with the OWNER's last-damage record owner+0x190 (B2:283473; the owner = slot +0x24 of 0x620000 = *(T+0x278) = the
    building, B2:187911) and third argument 0, then his deletion unless his collision type is 0x20 (B2:283468,
    283495-283496) (`re-vehicle-death.md` 10). BF1 `FUN_001c4ad0` (B1:70163-70193): per turret slot +0xc of the
    vtable at T+0x2a4 (0x534310; word 0x53431c = `FUN_002268a0` -> `FUN_002236a8` B1:139330) ->
    `FUN_0023ac68(T+0x10)` (B1:136981) - the same slot +0xc the BF1 vehicle death calls on each of its seats
    (`FUN_001ea190` B1:96784-96803; it calls `FUN_0023ac68` itself only on the vehicle's own +0x2a0, B1:96806).
4.2 **Live** [L] (`live/hot1b_leave.txt`, second run): 8 of 8 building deaths with a soldier seated killed him in that
    update - the 4 dish gunners (drawn, type 0x20) became corpses (alive bit clear, health -42 to -437, still
    EntitySoldier, falling), the 4 laser-tower gunners (hidden) were deleted (health 0, the slot no longer a soldier
    in the next update, reused later).
4.3 **The building's states** [V] (`re-build-zones.md` 1 [O]): death `FUN_001e3638` - with a destroyed model: state 1,
    the model K+0x384 (B2:93635) and the collision model E+0x70 := K+0x388, the destroyed mesh and prims
    (`FUN_00169af8` writes E+0x10+0x60, B2:93636); no destroyed model and an unbuilt one: state 2, K+0x4d4 and
    K+0x4d8 (B2:93601-93602); neither: hidden. Then regeneration = MaxHealth / RespawnTime (armed default 120 s);
    built again at full health (state 0, the class's model and collision).
4.4 **Live** [L] (`live/hot1_rows.jsonl`): 10 of 24 BF2 Hoth turrets went from state 0 to 1 in 240 s (three more,
    8, 13 and 16, were down when the run began); turret 16 and turret 23 went from state 1 back to 0 at 2,950 -> 3,000
    of 3,000 (RespawnTime 999999.9 regenerates 0.003 a second: they were repaired - WeaponRepair is among the last-hit
    weapons).

## 5. What the port does today (main 2d54802)

- `game/src/assets.rs:615-621`: a map instance's prim XFRM goes straight onto the instance matrix (PRNT taken as the
  identity), once, as a static world body (`Body.instance`, no owner). The geoturret's top half has no body.
- `game/src/vehicle_pads.rs:395-430` (`install`): a building turret is a `VehicleKind::Turret` with no hull parts; a
  bolt or rocket that meets its bodies is `Hit::World` (card VTHUL): its pool never falls.
- `core/src/sim/blast.rs:63-140`: the blast's LOS is `los_past(…, HullFilter::Static)`, which a building turret's own
  static bodies can block - the PS2 ignores the victim; the gunner rule (blast.rs:200-223) is the PS2's.
- `core/src/sim/seats.rs:685-694` (`carry_riders`): every rider stands at the vehicle's ORIGIN - a turret gunner's
  body is at the building's base, inside its bodies, not at his seat (the PS2: 1.35 - 1.59 m out on the dish).
- `core/src/sim/vehicles.rs:467-512` (`destroy_vehicle`): a destroyed turret is removed; its riders die (the PS2
  kills them too) but no destroyed model, collision, regeneration or rebuild follows (card TBLD).

## 6. For the port (build spec, card TCOL phase build)

1. **Bodies.** Each armed-building class's prims with the node they hang on and the role (turret / aimer / barrel /
   static; the map bake already keeps each prim's `parent` name); in the sim, a building turret's posed-node prims are
   movers (`CollisionWorld::with_movers`, the door path) placed each tick by the node matrices `re-turret-draw.md`
   2.2-2.3 gives (today in `game/src/viewer/turret_pose.rs`: move the pure node math into core), filed under the
   envelope of every yaw; the static-node prims and the mesh stay as they are.
2. **Ownership.** Every body of a turret's instance carries the turret vehicle as owner (VTHUL): a bolt / rocket / shell
   that meets one damages the pool through the HealthType building scale; a blast's LOS ignores the victim's own
   bodies; the gunner's own fire, aim ray and exit keep passing his building (`ray_past`, TURR).
3. **The gunner.** A drawn gunner's body at his seat (`L x node x O`, re-turret-draw 3.2; `GunnerHang`), hittable;
   a hidden gunner out of the collision world; damage to the building never reaches him; the death kills him with
   the building's last hit (corpse when drawn, gone when hidden).
4. **Death.** State 1 with the destroyed model and its collision (or the pad, state 2), regeneration MaxHealth /
   RespawnTime, built again at full health: card TBLD's state machine (batch, or TBLD first).
5. Acceptance (card): a scratch repro per class (BF2 GEO1's cannon first): rockets and bolts at the upper half stop
   on it N/N with the gun turned 0 / 90 / 180 degrees, the pool falls by the weapon's scaled damage, a drawn gunner hit
   by a bolt aimed at his seat loses the bolt's damage, a hidden one cannot be hit; the census above as a test over
   the baked classes (posed-node prims against the PS2's tops).

## 7. Still [H] / not read

- Which of the two gunner-only falls in 3.1 was a direct bolt (the code says it can be, 3.3).
- BF1's armed building collision getter is read (1.3) but no BF1 live run was made; the BF1 rows are RAM-only (3.2).
- The aimer's own update path into the table (FUN_003018d8's caller in the aimer tick) is cited from re-turret-draw
  2.3, not re-read here; the barrel recoil slide moves barrel prims too (re-turret-draw 2.3).
- A pad (state 2) taking damage (re-build-zones 1: [H] no).
- What the PS2 does to a gunner whose turret turns while he is inside the gun's own prims (the dish gunner sits inside
  `p_collision`'s 1 m cylinder on BF1).

## 8. Checks run and the verifier

- `re_check.py fn` on every cited line (`lead_tmp/tcol/checks/fn.txt`; three cites corrected after it: the grid test
  B2:58705, the deletion B2:283495-283496, the broadphase header B2:2152).
- `re_check.py word` and the disassembler on every constant, vtable slot, string and register hand-off
  (`checks/word.txt`: 0x61a458 / 0x61a45c, 0x62006c, 0x627308, 0x52ef08 / 0x52ef0c, 0x53431c, the three mesh strings,
  `bolt`, `laser`, `%s%d`; `lw a2, 72(s0)` / `lw a2, 68(s0)` at 0x160180; `addiu v0, a0, 0x298` / `0x1d8`; the mask
  immediates 0x132e / 0x13be at 0x30fbb4 / 0x30fbbc and 0x996 / 0x9de at 0x27320c / 0x273214).
- `re_check.py callers` for the twins (`checks/callers.txt`): `FUN_002f4e00` 8 callers in B2 (the vehicles' and the
  turret Controllables' slot functions), `FUN_0023ac68` 8 in B1, `FUN_0026c6d0` / `FUN_002236a8` one each (the turret
  slot thunks), `FUN_0026bf88` / `FUN_00222f20` 5 each; the vtable-only functions by ELF word (`vtab.py`):
  `FUN_0030fad0` in 9 ordnance vtables, `FUN_00273128` in 8, `FUN_00163770` at 0x60f074.
- No absence claim is made: the live and RAM rows carry every "only" ([L], scripts and outputs in section 9).
- **Verifier workflow** `tcol-re-verify` (run wf_1a4bebc3-52b; 5 agents at effort medium, claims and citations
  only): 27 claims in four batches; the 8 twin claims (BF1 does the same) each to TWO verifiers working apart.
  35 verdicts: 30 SUPPORTED, 5 PARTLY, 0 NOT SUPPORTED; the two twin verifiers agreed on all 8 (7 SUPPORTED, C20
  PARTLY from both), so the second verifier overturned none. Fixed after it: C10 (FUN_003018d8 HAS a decomp header,
  B2:293246), C19 (the rider kill's conditions: owner non-zero, cheat flags clear, type 4 -> 2 only when the pilot is
  not the Controllable), C20 (BF1's vehicle death reaches its seats through the same slot +0xc, not by calling
  FUN_0023ac68 per rider), C25 (10 of 24 turrets fell in the first run, not 11), C3 (the target-point queries run
  only with a Targetable subset; the ray path C28 added). C28 (the ray's per-object test reads the pose table,
  section 1.2) went to two more verifiers (run wf_9c77afa5-a5e, 2 agents): SUPPORTED by both, with two narrowings
  now in 1.2's reading - `FUN_00168318` moves the ray into the model's frame once (the inverse of the slot +0x48
  matrix) and, with its last argument set, tests only the bodies of the subset mask at model+0x128; body+0x48 is the
  key the prim loader passes as the hash of PRNT (`re-collision-layers.md` 1). Seven agents in all, 37 verdicts.
  Verdicts: `lead_tmp/tcol/checks/verify_wf.json`.

## 9. Tools and data (`engine/artifacts/lead_tmp/tcol/`)

- `primcensus.py bf1|bf2 [--all] [--model M] [--json F]`: every armed-building class's prims (NAME, PRNT, MASK with
  the carry-over, kind, size), its `coll` mesh and skeleton, each prim at the bind through its PRNT (the PS2) and at
  the identity (the port), the drawn top from the baked GLB (`glbbox.py`); outputs `census_bf1/2.txt|json`,
  `census_table.md`.
- `lists_b2.py STATE [--lvl X]`: BF2's collision world by list and type; each manned turret's building (state, health,
  nearest disc instance) and its seated soldier's lists. `gunner_bodies_b2.py STATE...`: the seated soldiers' place and
  collision sphere. `seats_b1.py`: BF1 seated soldiers by seat owner, list and type over every state.
- `gunner_live.py --state S --secs N --tag T` (PCSX2, SW renderer: `PCSX2_FORCE_SW=1` while the RTX is lost): one row
  per game update of every armed building's health / state / last-hit weapon and its gunner's, plus 4 s of each
  leaving gunner; `gunner_ana.py ROWS [END.p2s]` (building-only / gunner-only / both, by weapon type), `leave_ana.py
  ROWS` (deaths and their gunners). Runs: `live/hot1_*` and `live/hot1b_*` (from `lead_tmp/vturd/ps2/hot1_bots_battle.p2s`).
