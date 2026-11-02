# OPUS RE (loop lane 2, 2026-10-22, card VPOSW) - [O] UNAUDITED
# RE: a walker's posed nodes - its node table, the walk pose under its aimers and turrets, the aimers' own frame

**The questions** (card VPOSW, from `re-vehicle-pose.md` 6 and 8). A walker's aimers and MountedTurrets write their
nodes into the walker's node table, and the walker's skinned draw reads that table bone by bone. The port's walker
skin (`game/src/viewer/walker_pose.rs`) poses only the walk clip, so the AT-ST's head never turns toward its aim. Card
VPOSE's replay fitted the hovers. On walkers it failed (spider `turret_y_2` 0/37, AT-TE `aimer_rgun_1` 0/19, the
AT-TE's main-cannon turret node 0/19), and it left "which matrix a walker's aimer / turret node is built on" as [H].
Questions: (1) what sits in a walker's table and who writes each entry; (2) the parent step; (3) the frame a walker
aimer steers in; (4) the card's root-bob note: is the port's walker missing DummyRoot's translation?

**Method.** Read the walker constructors, the pose registration, the per-update compose, the draw and the aimer think
of both games (the BF2 aimer tick is missing from the export: read from the ELF words). Then read every walker's
node table in the 357 stored PCSX2 states at the walker's OWN offset (`lead_tmp/vposw/wtable.py`). Each entry is
labelled by the object its pointer lands in. Every node was replayed against the vehicle formulas of
`re-turret-draw.md` 2.2 / 2.3 with its parent read from that table (`wreplay.py`). A verifier workflow (6 agents,
claims and citations only; twin and absence claims read by two verifiers separately) judged 21 claims (section 10).

Tags: [V] read in the decomp / ELF at the cited lines, [L] measured in the PS2's RAM, [D] derived, [H] hypothesis;
[O] = load-bearing, lifted, unaudited. B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c`. E = a walker
entity, K = its class, T = a MountedTurret, A = an aimer, row vectors ("M1 x M2" applies M1 first).

## 0. Short answers

1. **Why VPOSE's walker rows failed** [L]: `vpose/ps2table.py` read every vehicle's table at the HOVER's offset
   (BF1 E+0x1d20, BF2 E+0x17e0). A walker's table is BF1 E+0x1ec0, BF2 E+0x1d7c. A hash table read 0x1a0 bytes early
   resolves only a few slots, so the walk-posed parents read as missing, and the replay fell back on their bind.
   Read at the right offset, every walker node fits (3). The same misread explains VPOSE's FLYER failures: BF1
   E+0x1ce0 and BF2 E+0x1940, 16 slots off. Read at the flyer's own table, the MAF's rear cannon fits 7/7 (VPOSE 0/7),
   the gunship ball turrets 24/24 (VPOSE 10/24), and all 1,501 flyer node rows fit (`ftable.py`, `freplay.txt`;
   card VPOSF).
2. **A walker's table holds the walk pose** [O][V][L]: the constructor registers a pointer to every bone of the walk
   pose's matrix block. The walker task recomposes that block every update. So a table entry of a walk bone IS that
   bone's walk-posed model matrix (the legs' 2h yaw on bone 0 included, bone 0's translation dropped). The aimers and
   MountedTurrets keep their own entries. The draw runs aimers, then turrets, then the model with the table.
3. **The parent step** [O][L]: a walker's aimer, turret and barrel nodes follow the vehicle formulas exactly, with the
   parent = the parent's TABLE entry: the walk-posed bone, or the matrix of a chained aimer or turret. BF1: 1,205 of
   1,206 rows fit to 1e-3 (worst 0.00000). The one miss is a barrel mid-recoil, slid 0.300 m along its parent's -Z.
   BF2: all rows within 0.04, 492 of 568 within 1e-3. BF2 writes aimer nodes at draw time, and a save state holds
   one update of motion: about 0.02 deg of direction and at most 0.009 m of walk pose. With the parent at its BIND
   instead: 0 of 888 BF1 rows and 0 of 444 BF2 rows.
4. **The aimer's steering frame on a walker** [O][V][L]: a body aimer whose base is the walker's world matrix steers
   its yaw / pitch, and is clamped to its limits, in the frame (its class bind rotation and position) x (its PARENT's
   table entry) x (the world matrix) (B1 FUN_0026d748 B1:191823-191900; BF2 tick 0x302380-0x3024bc). So the AT-ST
   head's yaw +-90 / pitch -5..20 deg are measured from cube5's bind under the walk-posed neck. Fits every BF1 body
   aimer row to 0.000 deg. The base matrix alone misses by 0.22 deg (AT-ST), 2.0 deg (spider) and 180 deg (AT-TE
   side guns). An aimer of a TURRET section that has a turret node steers in that turret's aim frame alone; a TURRET
   section without one (the AT-ST's ear guns, the AT-TE's side guns) gives its aimers the world matrix, as a body's.
5. **Two writers on one node: BF1's AT-AT head** [O][V][L]: `aimer_head` is TURRET1's TurretNodeName AND the BODY's
   AimerNodeName. The entry is the MountedTurret's own matrix, and at draw the body aimer writes over it in place.
   The drawn head is the body aimer's look-along: 24/24 at 0.00000, while the turret's own formula fits 0/24 (worst
   0.42). BF2's AT-AT has no MountedTurret: its head is a body aimer.
6. **The one-man AT-ST (BF2)** [L]: `aimer_y` (the BODY's TurretNodeName in the census) is drawn by the walk pose: its
   entry points at the pose storage and equals its bind under bone_root to 0.035 deg on 38 of 38 rows, 30 piloted.
   Its cockpit turns only through the aimer `aimer_x` (yaw -90..90, pitch -35..30 deg).
7. **The root bob** [L][D]: bone 0's translation is 0 in every walker's walk pose (348 of 348 rows). Every walker
   skeleton with a DummyRoot bone binds it at translation 0 (BF2's AT-TE skeleton has none: its pose rig's bone 0 is
   the bank's DummyRoot), and every DummyRoot clip track translates along z only (root motion).
   So the port, which holds DummyRoot at rest, matches the PS2 there. The bob is bone_root's own track (one-man AT-ST
   RAM y 2.700-2.936 against bind 2.942; its clips span 2.700-2.96). The 0.07-0.22 m that card VRIDE saw is not
   DummyRoot. Whether the port's bone_root equals the PS2's at the same clip and time is not measured [H] (the anim
   player E+0x1d70 was not read).

## 1. The census [L]

`wtable.py` finds walkers by their primary vtable: BF1 walker 0x533b18 and command walker 0x53e2b0; BF2 0x61f850 and
0x62be40. The command walkers (AT-AT, AT-TE) are built on the walker: B1 FUN_002a26e0 calls FUN_00215c88 first
(B1:230405), and B2 FUN_0035baa0 calls FUN_00259970 (B2:354294). Over the 357 stored states it reads 348 walkers with
a model: BF1 AT-ST 135 (jungle 27, snow 18), spider 37, AT-AT 24, AT-TE 19; BF2 AT-ST 44 (jungle 30, snow 6), spider
22, AT-AT 18, AT-TE 11, one-man AT-ST 38. It skips 108 more walker-vtable instances, the tauntauns (75) and kaadus
(33). They have no posed node in `vpose/nodecensus_<g>.txt`, so nothing in them is written. Labels over all entries:
POSE 7,434, bind 1,482, AIMER 1,302, BARREL 124, TURRET 54, other 0. Two verifiers checked separately: every bind
entry is an hp_* node equal to the skeleton bind, and every aimer and barrel object is reachable from its own walker.
Output: `wtable_all.txt`, `ps2/<state>_wtable.json`.

| class (games; showcase maps) | node: writer |
|---|---|
| AT-ST (both; BF1 NAB1 TAT1 YAV2, BF2 MYG1 HOT1) | cube5 (head, 851 v) and aimer_mouthpivot: body aimers; aimer_Reargun / aimer_Leargun: TURRET1's aimers in BF1 (BF2: Leargun the body's, Reargun TURRET1's; TURRET1 has no turret node, so their base is the world matrix); barrel_lgun / barrel_rgun: barrels of aimer_mouthpivot (BF1 written in place into the walk pose's storage; BF2 their own) |
| spider (both; BF1 YAV2) | turret_y_1 / turret_y_2 (yaw, pitch limits 0), turret_x_1 / turret_x_2 (pitch): body aimers |
| AT-TE (both) | aimer_maincannon_y: TURRET1's node (PitchTurnFactor 0); aimer_maincannon_x: its aimer (limits 0 / 0, base the turret's aim frame); aimer_Fgun_1..4: body aimers; aimer_Rgun_1 / 2: TURRET2's aimers (no turret node: base the world matrix) |
| AT-AT BF1 | aimer_head: TURRET1's node, written over by the BODY aimer (5); aimer_Lgun / aimer_Rgun: TURRET1's aimers; barrel_Lcannon / barrel_Rcannon: the body aimer's barrels, in place into the walk pose storage |
| AT-AT BF2 (HOT1) | no MountedTurret; body aimers aimer_head -> barrel_Lcannon, barrel_Rcannon (aimers despite the names); barrels aimer_Lgun / aimer_Rgun |
| one-man AT-ST BF2 (UTA1 MYG1) | aimer_x: body aimer; aimer_y: the walk pose |

Limits per aimer: `waimers.txt` (AimerYawLimits / AimerPitchLimits from the class record). Every walker aimer runs at
the default rates of 57.3 deg/s pitch and 114.6 deg/s yaw: BF1 class+0x30c / +0x318 (defaults B1:192338-192386), BF2
+0xcc / +0xd8 (FUN_00302d80). `drawn_census.py`: every drawn bone of every walker model is in its table, and no
drawn bone hangs unwritten below a written node.

## 2. The table [O]

- [V] **BF1** constructor FUN_00215c88 (B1:127445). The table is E+0x1ec0 (B1:127485 *(decompiler detail omitted)*),
  with keys cleared and count zeroed at B1:127518-127519. Each MountedTurret gets the table (B1:127576
  FUN_00222f20). Each own-weapon aimer gets base E+0x200 and the table (B1:127643 FUN_0026cab0). Then
  `FUN_00497f28(table, E+0x16a0)` (B1:127716), then the bind fill FUN_004969d0 (B1:127717). FUN_00497f28 reads the
  pose data through E+0x16a0 = E+0x690 (FUN_0043ec30 at B1:127677). For each pose bone it inserts a pointer to
  E+0x690+0x10+0x40 i when the key is new (B1:558743). When the key is present it COPIES the bone's matrix into the
  existing entry and leaves the pointer alone (B1:558746-558774). Turret and aimer nodes are inserted first, so they
  keep their own matrices (the first inserter wins; an insert never overwrites).
- [V] **BF2** constructor FUN_00259970 (B2:175401): table E+0x1d7c (*(decompiler detail omitted)*), keys cleared B2:175471;
  turrets B2:175523; aimers `FUN_00301588(.., E+0x270, E+0x120, table, 0)` B2:175591; compose
  `FUN_00533f20(E+0x550, 0x73d8c0)` B2:175723; `FUN_0057e988(table, E+0x550)` B2:175724; bind fill B2:175725.
  **BF2 re-points instead of copying.** For each pose bone FUN_0057e988 REMOVES an existing entry (FUN_00539220,
  B2:714162-714164) and inserts a pointer to E+0x550+0x10+0x40 i (B2:714166). A turret node that is also a pose bone
  (the AT-TE's aimer_maincannon_y) therefore starts on the walk pose. The first turret tick re-points it
  (B2:189497-189519, `re-vehicle-pose.md` 2.2), and the first draw re-points each aimer node (B2:293451-293469).
- [V] **Every update** the walker task recomposes the pose block: BF1 FUN_0021b3a0 calls `FUN_00440b38(E+0x690,
  0x66ef20)` (B1:131561); BF2 FUN_00261208 calls `FUN_00533f20(E+0x550, 0x73d8c0)` at B2:180843, between slot +0x138
  and slot +0x140, which put on and take off bone 0's 2h yaw and bone_root's pitch / roll (`re-walker.md` 10). (A
  verifier notes that FUN_0021b3a0 returns early on a network path and in one jump state, B1:131446 / 131458.)
  `re-walker.md` 17 cites B2:180841, which is the line before; corrected there.
- [V] **No re-registration** [receipt:0cfa3736] [receipt:9f619cff]. The only walker-code call of B1 FUN_00497f28 or
  its twin FUN_00498040 is the constructor's. In BF2 the only walker-code call of FUN_0057e988 / FUN_0057eaa0 /
  FUN_0057ea80 is B2:175724. Both verifiers found the ELF `jal` counts equal to the export's and no data-word
  references. The tables are cleared only in the constructors ([receipt:14ce016e] [receipt:450bf128], VPOSE).
- [V] **The draw.** BF1 FUN_00217278, through the walker's thunk FUN_0021fb10 (B1:134554) and the command walker's
  FUN_002a2ef8 (B1:230833), runs in this order:
  - returns without drawing when its own slot-+0x30 test of E+0x558 fails (B1:128427-128430), or when any
    MountedTurret's test of T+0x2c8 fails (B1:128432-128443);
  - each own aimer `…
  - each MountedTurret `FUN_002255b0` (B1:128496), which copies T+0x320 into whatever the turret node's entry points
    at (B1:138447-138482), then refreshes the turret's aimers (B1:138487-138495);
  - the model with the table (B1:128592-128593).

  BF2 FUN_0025b428 (thunk FUN_00267118, B2:184709; its address is a data word at 0x61f7d4 and 0x62bdc4):
  - aimers B2:176748;
  - turrets B2:176954, only while the fade value is at least 0.8;
  - the model B2:176971.
- [V] The second per-walker object the constructor hands the table to is the PassengerSlots object: key 0x64c511a4,
  B2 setter B2:183374-183389, K+0xd50 / K+0xd54 <= 4, constructor B2:175554; BF1 K+0x2690 B1:127606. [L] No table
  entry points into it.

## 3. The nodes: the parent is the table entry [O][L]

`wreplay.py --alt` (`wreplay_all.txt`, every walker class) uses the formulas of `re-turret-draw.md` 2.2 / 2.3 on each
written entry, with the parent read from the walker's own table:

| game | population | rows | fit to 1e-3 | parent at its bind |
|---|---|---|---|---|
| BF1 | aimer nodes (AT-ST x4 incl. jungle / snow, spider x4, AT-TE x7, AT-AT guns x2) | 869 | 869 (worst 0.00000) | 0 |
| BF1 | AT-TE aimer_maincannon_y, turret formula (PitchTurnFactor 0) | 19 | 19 | 0 |
| BF1 | barrels: rotation = parent's, position = bind offset x parent | 318 | 317 (one slid 0.300 m along the parent's -Z: recoil) | - |
| BF2 | aimer nodes | 433 | 357 (all within 0.04; most misses 0.007-0.008) | 0 |
| BF2 | AT-TE aimer_maincannon_y turret / barrels | 11 / 124 | 11 / 124 | 0 |

- [L] The BF2 residual splits into the aimer's direction (node Z against A+0x50, up to 0.024 deg) and the pivot
  (against L.row3 x parent, up to 0.0094 m; `wdiag.py`). BF2 writes aimer nodes at draw time. A save state holds
  one think and one walk-pose update newer than the draw: the hovers' 0.01 of `re-vehicle-pose.md` 3.
- [L] With the parent's BIND instead of its entry, no row of any population fits. That was VPOSE's failure.
- [D] So a walker's head and guns ride the walk animation through their parents (the AT-ST's cube5 hangs from the
  walk-posed bone_head), and they turn by the aimer's direction from there.

## 4. The BF1 AT-AT head [O][V][L]
- [V] Constructor order puts TURRET1 first, so `aimer_head` points at T+0x320. The body aimer's node write
  (FUN_0026ce10, in place, `re-vehicle-pose.md` 2.2) at B1:128485 lands in T+0x320. FUN_002255b0 then copies T+0x320
  into the same entry (B1:138447-138482), a self-copy. The turret's own formula is recomputed in its next tick, after
  this draw.
- [L] `atat_head.py`: the turret formula fits 0/24 (|dM| 0.0025-0.42; it fits only near yaw 0); the BODY aimer's
  formula fits 24/24 (0.00000). The body aimer's own node matrix A+0xc0 is unused (14-16.5 off the entry).
  TURRET1's aimers (aimer_Lgun / aimer_Rgun) hang from that body-aimed head (24/24).
- [D] On screen the BF1 AT-AT's head follows the DRIVER's aim, and its chin guns the gunner's.

## 5. The aimer's own frame on a walker [O]
- [V] BF1 FUN_0026d748 (the aimer think). When the table A+0x100 is set, A+0x24 is the owner's matrix getter's result,
  the class's parent hash (+0x354) is non-zero and the parent's entry is found (B1:191823-191827), it builds
  mat(class bind quaternion +0x340, position A+0x30) x the parent's entry (B1:191828-191857). It keeps row 3 at A+0x60
  and the rotation at A+0x70 (B1:191858, 191864-191868). In every case the frame is mat(A+0x70, A+0x60) x *(A+0x24)
  (B1:191871-191900). FUN_0026dd70 (B1:192006) clamps the pitch A+0x40 to class+0x304..0x308 and the yaw A+0x48 to
  +0x310..0x314 (B1:192166-192199; a negative yaw wraps by 2 pi first when +0x31c is set). It then writes A+0x50 = sin
  y cos p F0 + sin p F1 + cos y cos p F2 (B1:192202-192217). The verifier's PARTLY on C10: when the owner's seat holder
  has its +0x230 set, that holder's vtable +0x78 replaces the four limits, and when the owner's +0xe8 >= 0 the
  function returns before the clamp and the write (B1:192157-192163). These are the player / AI seat paths, not read
  further here.
- [V] BF2, the aimer tick missing from the export at 0x3021f8 (`lead_tmp/vposw/b2_aimer_tick.txt`, `eedis.py`),
  MIPS 0x302380-0x3024bc. The same tests: A+0x210 table, A+0x24 against the owner's getter, class+0x114 parent hash,
  FUN_00539150. The same build: FUN_0053ac20(class+0x100, A+0x30) x entry, row 3 -> A+0x60, FUN_0053a158 -> A+0x70,
  then FUN_0053ac20(A+0x70, A+0x60) x *(A+0x24). FUN_00302a10 (called at 0x3027cc) is FUN_0026dd70's twin: limits
  class+0xc4 / +0xc8 / +0xd0 / +0xd4 (B2:293876-293879), A+0x50 at B2:293956.
- [L] `wbodyframe.py`: that frame fits every BF1 world-based aimer row to 0.000 deg (AT-ST 4 x 90 plus jungle /
  snow, spider 4 x 37, AT-TE 6 x 19) and BF2 within 0.003 deg on most rows. The worst are 0.115 (AT-AT cannons)
  and 0.728 deg (one-man), from the draw-time lag. The base alone misses (a median 0.22 / 2.02 / 179.9 deg). The
  aimers of a TURRET section with a turret node fit their turret's aim frame alone (the base test fails, so
  A+0x70 / A+0x60 keep their constructor values): AT-AT guns 24/24, AT-TE maincannon_x 19/19.
- [D] For a hover or a building whose aimer parent is static, this frame is a constant rotation of the base, so
  re-turret-draw 2.3's "the base matrix" holds where it was measured. On a walker the frame sways with the walk.

## 6. What turns on screen, per walker [D from 1-5]
- AT-ST: the head cube5 looks along its aimer's direction, within yaw +-90 and pitch -5..20 deg of its bind under
  the posed neck. The chin pivot (pitch -30..20, no yaw), the two chin barrels and the two ear guns hang from it.
- Spider: two turrets, each a yaw aimer (no pitch) and a pitch aimer under it.
- AT-TE: the main cannon yaws with TURRET1's gunner (PitchTurnFactor 0) and its barrel pitches on the turret's aim
  frame. Four front guns (body aimers: the driver) and two side guns (TURRET2's gunner).
- AT-AT: the head follows the driver's aim (BF1: through the body aimer over TURRET1's node; BF2: a body aimer), the
  two cannons with it; the chin guns follow TURRET1 (BF1) or are the head aimer's barrels (BF2).
- One-man AT-ST: the cockpit aimer_x looks along the pilot's aim (yaw +-90, pitch -35..30); aimer_y stays on the
  walk pose.
- A walker never aimed (fresh, yaw = pitch = 0) looks along its nodes' bind axes under the walk pose: the port's
  present picture.

## 7. For the port (card VPOSW, build)
What exists: the walk palette (`WalkerRig::palette`, `node_now`), the node laws `turret_pose::{turret_node,
aimer_node, barrel_node}`, the seat look and aim (`core/src/vehicle/gun.rs`), the per-seat first aimer's limits
(`vehicle_class.rs`).
1. **Bake / class data**: per walker class, each section's aimer chain is needed: the node, its parent, AimerYawLimits /
   AimerPitchLimits, the rates, whether its base is the body (BODY section) or a turret's aim frame (TURRET section),
   and its barrels. The node bind locals come from the skin GLB's skeleton.
2. **Pose**, each drawn tick, after `WalkerRig::palette`. Walk the written nodes parent-first. A parent's matrix now =
   its walk-posed model matrix (`palette[p] * global_bind[p]`), or the written matrix of a chained aimer / turret.
   - aimer: `aimer_node(parent_now, local, dir_model)`;
   - turret node: `turret_node(parent_now * local, yaw, pitch x PitchTurnFactor)`;
   - barrels: `barrel_node`.

   Write each back as `palette[n] = node * global_bind[n]^-1` before skinning (one bone per vertex: every drawn bone
   is in the table, section 1). BF1's AT-AT head takes the BODY aimer's matrix (4). Visual only: no sim hash.
3. **Directions**: a body aimer's direction is clamped in the frame bind(class) x parent_now x body (5). Its want
   direction is toward the seat's aim point; the PS2 steers its muzzle line through the point at the class rates
   (re-turret-draw 2.3). PROVISIONAL: the port has one aim per seat, so the steady state (clamped, rate-free) is the
   first build and the rates are a follow-up. A turret-section aimer uses its seat's aim frame. A never-aimed walker
   keeps the walk pose.
4. **Acceptance data**: `lead_tmp/vposw/walker_nodes_ps2.txt` holds 1,798 RAM rows. Each row has the parent's entry,
   the bind local, the direction in model space (turret rows: yaw and scaled pitch) and the PS2's entry. A numpy pass
   reproduces BF1 1,230 / 1,230 and BF2 492 / 568 at 1e-3 (all within 0.04). Mutation: the parent at its bind fails
   every row.

## 8. Still [H] / not read
- Whether the port's bone_root equals the PS2's at the same clip and time (the bob, 7). Read the anim player E+0x1d70
  (`re-walker.md` 1) in a state and sample the port's clip there.
- The seat paths of FUN_0026dd70 (the holder's limit override, the early return when the owner's +0xe8 >= 0).
- Which aim point a walker's body aimer chases (the owner's +0x10, not read for walkers); the muzzle-line steering of
  re-turret-draw 2.3 is assumed to be the same code.
- The whole-draw hide (2): its viewer test (B1 FUN_00283d18, BF2 slot +0x38) is the first-person view's (card FPV),
  not read.

## 9. Tools and data (`engine/artifacts/lead_tmp/vposw/`)
`wtable.py` (every walker's table at its own offset; `wtable_all.txt`, `ps2/*_wtable.json`), `wreplay.py` and
`wreplay_lib.py` (the node formulas; `wreplay_all.txt`), `wdiag.py` (BF2's residual split), `atat_head.py`,
`waimers.py` (limits, rates, base; `waimers.txt`), `wbodyframe.py` (the frame candidates; `wbodyframe.txt`),
`drawn_census.py` (drawn bones against written nodes), `wclip/` (a bank's clips with duplicate tracks and per-bone
translation ranges), `eedis.py` (an ELF range by word; `b2_aimer_tick.txt`), `ftable.py` and `freplay.txt`
(flyers at their own offset, for VPOSF), `mkrows.py` and `walker_nodes_ps2.txt` (the build's rows),
`claims/claims.json`, `verify/` (the workflow's verdicts and its own checks).

## 10. Checks run and the verifier
**Self-checks** (`lead_tmp/vposw/checks/`):
- `re_check.py fn` on every cited line (`fn.txt`, each in the function named). The run caught B2:180841 (re-walker
  17), which is the line before the compose.
- `re_check.py callers` for FUN_00497f28, FUN_00215c88, FUN_00259970, FUN_0025b428, FUN_0029f0a0, FUN_00277f80.
- `re_check.py absent` [receipt:0cfa3736] [receipt:9f619cff] (`absent_posefill.txt`).
- The key hash 0x64c511a4 resolved by FNV against the key dictionaries.
- The BF2 aimer tick listed from the ELF words.

**Workflow `vposw-re-verify`** (6 agents, effort medium; claims and citations only, `claims/claims.json`,
`verify/verdicts.json`):
- Two verifiers read the 8 plain code claims in two batches: 7 SUPPORTED and 1 PARTLY (C10: the seat paths of
  FUN_0026dd70, folded into 5).
- Two verifiers read the 5 twin and absence claims separately: 5 / 5 SUPPORTED by both, none overturned. Both did
  their own ELF `jal` / data-word scans for C12.
- Two verifiers re-ran the 8 RAM claims separately: 6 SUPPORTED, 2 PARTLY by both. L1: `wtable.py` skips 108
  tauntaun / kaadu instances with no model; reworded in 1. L3: the first stored replay output left out the
  jungle / snow AT-STs, where one BF1 barrel row is mid-recoil and BF2's worst rows are 0.007-0.008; replaced by
  `wreplay_all.txt` in 3.

What the verifiers found that this session had not:
- the recoil row;
- the BF2 turret loop's fade gate (>= 0.8);
- FUN_0021b3a0's early returns;
- FUN_0026dd70's seat paths;
- BF2's AT-TE skeleton has no DummyRoot bone (its pose rig's bone 0, `4446e9b8`, is the bank's DummyRoot);
- every walker bank's DummyRoot tracks translate along z only (both games, 9 banks).
