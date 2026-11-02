# OPUS RE (loop lane 2, 2026-10-22, card VPOSE) - [O] UNAUDITED
# RE: a vehicle's own posed nodes - the node table, its turret and aimer nodes, the FinAnimation nine-pose

**The questions** (card VPOSE, from card VRIDE's RE: `re-vehicle-riders.md` 5, 2.2). The port draws every hover
rigid in its bind pose (`game/src/viewer/vehicles.rs`): a fighter tank's turret never turns with its gunner's aim
and a speeder bike's vanes never move. What does the PS2 draw? (1) Which of a vehicle's nodes move, and by which
code: its TURRET sections (`TurretNodeName` / `AimerNodeName` / `BarrelNodeName`), its own weapons' aimers, the
class's `FinAnimation` clip. (2) How the FinAnimation clip is sampled from the drive's numbers. (3) In which order
the writers run, and what a rider hung from a posed node sees. (4) Whether every such model splits into rigid parts.

**Method.** The building-turret RE (`re-turret-draw.md` 2, [O]) already read the turret node, the aimer node and the
barrel law and the shared node table of an armed building. This pass read how a VEHICLE owns those writers (its
constructors, switch-on, draw), the FinAnimation path end to end (setter, bank, constructor, drive, sampler, blend,
compose, table fill), the flyer's fin path and the task order, with two reader agents (`lead_tmp/vpose/AGENT_A.md`,
`AGENT_B.md`; every citation re-checked). Then every rule was replayed on the PS2's own RAM: the node table of every
pad vehicle in the 337 stored PCSX2 states of both games (254 hold one) plus 18 bike-ride states of card VRIDE
(`ps2table.py`), the FinAnimation pose against the clips decoded from the disc (`replay_fin.py`), the turret node
(`replay_turret.py`) and the aimer nodes (`replay_aimer_veh.py`). A census over the raw LVLs of both discs lists every
vehicle class's posed nodes and its model's triangles per bone (`nodecensus.py`). A verifier workflow (7 agents,
claims and citations only; every twin and absence claim read by two of them) judged 40 claims (section 10).

Tags: [V] read in the decomp / ELF at the cited lines, [L] measured on the running PS2 or in its RAM, [D] derived,
[H] hypothesis; [O] = load-bearing, lifted, unaudited. B1 = `SWBFRONT.ELF-decomp.c`, B2 = `SWBFRNT2.ELF-decomp.c`.
E = a vehicle entity, K = its class, T = a MountedTurret, A = an aimer. A matrix is 4 rows (X, Y = up, Z = forward,
translation); "M1 x M2" applies M1 first (row vectors, as `re-turret-draw.md`).

## 0. Short answers

1. **Every vehicle has one node table, and its draw reads it** [O][V][L]: BF1 E+0x1d20 (hover; walker E+0x1ec0,
   flyer E+0x1ce0), BF2 E+0x17e0 (walker E+0x1d7c, flyer E+0x1940) - a hash map of node CRC -> pointer to a
   model-space matrix, the same structure an armed building has. The vehicle hands it to every MountedTurret of its
   TURRET sections and to every aimer of its own weapons, and draws each segment by its node's entry. Nothing clears
   it [receipt:14ce016e] [receipt:450bf128]: an entry keeps its last matrix until a writer writes again. A node no
   writer owns stays at the skeleton's bind.
2. **Three writers, and only these, move a hover's nodes** [O][V][L]:
   - the **MountedTurret** of each TURRET section writes its `TurretNodeName` node (yaw, pitch x `PitchTurnFactor`,
     re-turret-draw 2.2), its aimer nodes and barrels (2.3) - in RAM the AAT's `Gun_Body`, the fighter tanks'
     `turret_y`, the BF2 combat speeder's `aimer_missle_y` equal that formula on 362 of 362 turret rows;
   - the **aimers of the vehicle's own weapons** (the BODY's `AimerNodeName` / `BarrelNodeName`): the fighter tank's
     side guns `lgun_x/y`, `rgun_x/y` and barrels `Turret_1/2`, the AAT's `side_gun_1/2`, the bike's `gun`, the
     BARC's front guns, the combat speeders' guns - each a look-along matrix of its own aimer's direction (2.3's
     formula on every BF1 row, BF2 within 0.01);
   - the **FinAnimation nine-pose** (`FinAnimation` names a clip in the class's `AnimationName` bank): the drive
     samples it at frame a and frame b, blends by c (the rider's numbers: a = y + 4, b = y + (p > 0 ? 1 : 7),
     c = |p|) and writes the clip rig's bones into the table - 297 of 297 driven and parked hovers in RAM fit to
     0.001 deg (bikes, fighter tanks, BARC).
3. **What the fin clips move** [L]: fighter tank skis (45 deg) and winglets (95 deg); speeder bike handlebars (60),
   foils (50), rear flaps (100), foot pedals (40) and the whole body bone `bone_root` (+-5 deg pitch, 4 cm up and
   down); BARC its body `boneroot` (9.5), pedals (87), handles (63); skiff rudders (80) and levers (17); snowspeeder
   flaps (55 / 52, a flyer, its own numbers). No fin clip touches a turret, aimer or barrel node. Frame 4 is the
   bind pose in every clip, so a parked, never-driven hover is drawn exactly as the port draws it now.
4. **The order** [O][V]: the drive (and its fin fill) runs at task key 0, the hover's physics tick (which seats the
   pilot) at 15, the turret tick at 20 + HierarchyLevel, the aimers after it; the draw refreshes aimers and turrets
   again. So a turret or aimer node is always the turret's / aimer's matrix, and a rider hung from a fin-posed node
   follows that node's pose of the same update: **the BARC's rider leans with `boneroot`** (in RAM: hp_rider's bind
   x boneroot's fin entry, up (0, 1, -0.010)); the fighter tank's gunner rides `turret_y` (re-vehicle-riders 3).
5. **The rigid-part question** [L]: of the hovers' models, 0 triangles span two bones in every class but BF1's
   combat speeder (14 of 1,431); the BARC (16), the walkers and some flyers have spanning triangles. One-bone skinning
   (the walker skin's path) draws every one of them exactly; per-node rigid parts draw all hovers but two.
6. **A port bug found on the way** [V][L]: the hover's smoothed p and y start at **0**, not -1: the constructors
   zero them (B1:96395-96396, B2:130826-130827) and write -1 only to a, b, c; 786 of 786 never-driven hovers in RAM
   read p = y = 0. The port's `turret_pose::HoverNine::START` is (-1, -1), so the port's rider eases out of a
   full-reverse, full-left pose for about half a second after boarding a fresh hover. `re-vehicle-riders.md` 4.1
   said "both numbers start at -1"; corrected there.

## 1. The census [L - counted by script over the raw LVL chunks]

`lead_tmp/vpose/nodecensus.py bf1|bf2` (vride/vehcensus.py's loader) lists, for every vehicle class with a posed
node or a FinAnimation (BF1 31 of 43 vehicle classes, BF2 43 of 57), each section's `TurretNodeName`,
`AimerNodeName`(s), `BarrelNodeName`(s), `PitchTurnFactor`, `HierarchyLevel`, the BODY's `FinAnimation`, the model's
vertices per bone and the triangles across bones (`vturd/softcensus.py`'s rule), and whether the class name is in a
showcase map's LVL. Output: `nodecensus_bf1.txt`, `nodecensus_bf2.txt`.

Drawn geometry that moves, on the showcase maps (vertices on posed nodes / all; spanning triangles):

| game | class (maps) | turret section nodes | own-weapon aimer nodes | FinAnimation bones | spanning |
|---|---|---|---|---|---|
| both | rep_hover_fightertank (BF1 NAB1 TAT1 YAV2, BF2 UTA1 MYG1), imp_ (BF2 UTA1 MYG1) | turret_y (124 v), turret_x (150), turret_barrel1 | lgun_x/lgun_y, rgun_x/rgun_y, barrels Turret_1/2 | skis, winglets | 0 / 2,109 |
| both | cis_hover_aat (BF1 NAB1 TAT1, BF2 UTA1 MYG1) | Gun_Body (546), Main_Gun (145), recoil_3 (100) | side_gun_1/2 (126 each), recoil_1/2 | - | 0 |
| BF1 | all_hover_combatspeeder (NAB1 TAT1 YAV2), model all_hover_rebel_attack_speeder | aimer chain turret_body (103) -> turret_aimer (38), barrel turret_barrel | gun_left/right (74/76), barrels | - | 14 / 1,431 |
| BF2 | all_hover_combatspeeder (UTA1 MYG1), model all_hover_tank | aimer_missle_y (82) -> aimer_missle_x (1,336) | hp nodes only (0 v) | - | 0 |
| both | imp/rep_hover_speederbike (BF1 NAB1 TAT1, BF2 MYG1) | - | gun (112, under bone_root) | bone_root and 8 vanes / bars: all 2,421 v | 0 |
| BF2 | rep_hover_barcspeeder (MYG1) | - | aimer_frontleft/right_gun (211 each) | boneroot, pedals, handles: all 2,548 v | 16 / 1,653 |
| BF1 | tat_hover_skiff (TAT1), model all_hover_skiff | gunl, gun, gunr (283 each, no aimer) | - | rudders, levers | 0 |
| BF1 | cis_hover_stap (NAB1 TAT1) | - | hp_gun_1/2 (0 v: nothing drawn moves) | - | 0 |

Walkers and flyers on the showcase maps carry posed nodes too (section 6): the AT-ST's head `cube5` (851 v, BF1 NAB1
TAT1 YAV2, BF2 MYG1 HOT1) and its chin and ear guns, BF2's one-man AT-ST (`aimer_y` / `aimer_x`, UTA1 MYG1), the
AT-AT's head (HOT1), the spider walker's guns (YAV2), the gunships' wing and ball turrets (MYG1), the snowspeeder's
flaps and tow-cable gun (HOT1).

No drawn bone of a hover hangs, unwritten, under a written node (a child the PS2 would leave at its bind while its
parent moves) - checked per model from the census and the clip rigs; the one hit, `bone_root` under the fighter
tank's fin-posed `dummyroot`, is harmless because the clip holds `dummyroot` at the identity.

## 2. The node table [O]

### 2.1 Layout and lookup
- [V] A table is `+4` count, `+8` the map: FUN_00444998 (B1:504267-504290; BF2 FUN_00539150 B2:668115) takes 0x100
  >> 1 = 128 key words and the 128 value words after them, starts at key & 127, probes downward with wrap, stops on
  a 0 key. Key = the node's engine CRC (`re-anim-banks.md` 2.3), value = a pointer to a 64-byte matrix. Insert
  FUN_004449f8, BF2 FUN_005391b0; BF2 also removes (FUN_00539220).
- [L] `ps2table.py` reads it in a save state and names each key by the CRC of the model's `skel` bones
  (`skel.py`): every entry of every pad vehicle resolves.

### 2.2 Who puts what in it
- [V] **The constructor**: BF1 FUN_001e9428 takes E+0x1d20 (B1:96283 *(decompiler detail omitted)*), creates each
  MountedTurret with it (`FUN_00222f20(alloc, owner, class, table)`, B1:96320; the turret keeps it at T+0x3a0,
  B1:136665) and each own-weapon aimer with it and the world matrix E+0x200 (`FUN_0026cab0(.., E+0x200, E+0x1d20,
  0)`, B1:96410; A+0x100, and it inserts a pointer to its node matrix A+0xc0, B1:191047-191055); writes the
  FinAnimation palette into it (B1:96482) and the skeleton's bind for every other node (FUN_004969d0, B1:96487).
  BF2 FUN_00219918: E+0x17e0 to FUN_0026bf88 (B2:130748; T+0x280, B2:187898; a pointer to T+0x200 inserted,
  B2:188092-188098) and to … palette B2:130947; bind B2:130952.
- [V] **No reset** [receipt:14ce016e] [receipt:450bf128]: keys and count are written only in the constructors
  (B1:96286-96289, B2:130708). An entry is re-pointed or overwritten by its writers and otherwise keeps its matrix.
- [V] **The two games write differently**: BF1 writes IN PLACE - the turret tick (B1:137641-137672) and the fin
  fill FUN_00498040 (B1:558786-558852) insert a pointer to their own matrix when the node is missing, else copy into
  whatever the entry points at; BF2 RE-POINTS - the turret tick removes a foreign entry and inserts T+0x200
  (B2:189497-189519), FUN_0057eaa0 removes and re-inserts every palette bone (B2:714188-714215), the aimer FUN_003018d8
  likewise (B2:293451-293469). [L] In RAM both end the same way: the turret node points at T+0x320 / T+0x200, each
  aimer node at its aimer object's node matrix, each fin bone at the hover's palette (BF2 E+0x620 + 0x40 i, BF1 the
  palette E+0x740 + 0x40 i), every other node at the skeleton's shared bind.

### 2.3 The order in a frame
- [V] Tasks run by key, ascending (`ps2-frame-task-order`: BF1 FUN_001971c8, BF2 FUN_001af7e8): the hover's
  switch-on registers its controllable - whose update runs the drive and the fin fill - at key 0 and its physics
  task at 15 (BF1 FUN_001efa78 B1:101063-101064; BF2 FUN_002216b8 B2:136420-136421), then switches its turrets and
  aimers on: the turret tick at key **20 + K+0x1120** (HierarchyLevel; BF1 FUN_00225478 B1:138359; BF2 FUN_0026f3c8
  B2:190307, K+0x820; a turret class with `Virtual` set registers no node at construction, B2:188084-188094) and each
  aimer at 20 + its CLASS record's HierarchyLevel field (*(A+0x1c)+0x360 / +0x11c) in list 1, then its chained aimer
  (B1:192234, B2:293973).
- [V] The draw (BF1 FUN_001ea2b8 via the vtable word 0x531524 = 0x1f2468; BF2 FUN_0021a968 via 0x61cbf4 = 0x224848)
  refreshes every own aimer (B1:97095 / B2:131729) and every MountedTurret (B1:97106 / B2:131744) with the table, then
  draws the model with it (B1:97179 / B2:131847). BF1's aimers also write at think time (FUN_0026d748 B1:192105).
- [D] So in an update: fin pose (key 0) -> pilot seated on the posed table (key 15, the tick's seat call,
  re-vehicle-riders 2.2) -> turret node and its gunner (20+) -> aimers -> draw. The fin clip leaves turret and
  aimer nodes alone (section 4.4: the clips' tracks are fin bones only), and a turret whose parent the fin clip moved
  would follow it (its bind position and Y are carried through the parent's entry, re-turret-draw 2.2): the hover
  turret nodes' parents (`nodecensus_<g>.txt`) are bone_root, boneroot, baseroot, the model root or - the skiff's
  three guns - dummyroot, which the skiff's clip holds at the identity.
- [V] BF2's draw has a second fin pass (B2:131752-131795, FUN_0057ea80 -> FUN_0057eaa0) for a networked replica: it
  needs a pilot and both class words as the drive does, and DAT_0060050c == 0 and (DAT_0060054f ? DAT_00600549 :
  DAT_00600548) != 0, written by the network code (`re_check.py writers`); [L] the four bytes are 0 in the BF2 states
  read (`checks/roots_ram.txt`). Offline it never runs.

## 3. The turret and aimer nodes on a vehicle [O][L]
- [L] `replay_turret.py` over every table dump: each hover `TurretNodeName` entry points at its turret's own matrix
  and equals re-turret-draw 2.2's formula from the turret's yaw (BF1 T+0x390, BF2 T+0x270) and pitch x
  `PitchTurnFactor` (BF1 T+0x388, BF2 T+0x268): BF1 cis_hover_aat 56/56, rep_hover_fightertank 31/31,
  imp_hover_fightertank 25/25; BF2 cis_hover_aat 45/45, rep 26/26, imp 88/88, all_hover_combatspeeder 91/91. "No
  yaw", "yaw negated" and "full pitch" fail on every row with a yaw (e.g. the AAT: 17 of 56). Output
  `replay_turret.txt`.
- [L] `replay_aimer_veh.py`: each entry that points at an aimer object's node matrix (BF1 vtable 0x539258, node A+0xc0;
  BF2 0x625f88 at A+0x14, node A+0xd0) equals re-turret-draw 2.3's aimer-node formula from the aimer's world direction
  (A+0x50) in the vehicle's frame, the bind local of the node and its parent's entry: BF1 on every row (to 1e-5: the
  fighter tanks' five aimers 31/31 and 25/25, the bike's gun 197/197, the AAT's 56/56, the combat speeder's 52/52); BF2
  within 0.01 on every row, within 1e-3 on 75-100 % per node (lowest the BARC's right gun 3/4, the bike's gun 143/182) -
  BF2 writes aimer nodes at draw time only, so a save state between the think and the draw sees one update of motion.
  Barrels (BF2 A+0x110 + 0x40 i): their parent's rotation on every row. Output `replay_aimer_veh.txt`.
- [L] The card's acceptance number: the BF2 Mygeeto fighter tank's turret (`ref/states/bf2_myg1_cp2_cp1_con_post1_
  spawn.p2s`, yaw -0.0584, PitchTurnFactor 0) turns `turret_y` (bind (0, 1.7932, -2.8576) under bone_root) by 3.35
  deg; `hp_active` (bind (0, 0.658, -3.010)) through it lands at (+0.0089, 0.6581, -3.0098) - the gunner's RAM place
  (0.0089, 0.6581, -3.0100) of re-vehicle-riders 3.
- [V] A turret with no gunner keeps its last angles (the stick terms are 0; the constructor starts both at 0
  or the middle of their limits, B1:136679-136688). [D] A parked tank's turret faces its last aim.

## 4. The FinAnimation nine-pose [O]

### 4.1 The key, the bank and its rig
- [V] `AnimationName` (0x98557a5e) looks up a global bank by the case-folded FNV of the value (BF1 FUN_001f1fe8
  B1:102596-102598 -> K+0x25f8; BF2 FUN_00224320 B2:138123-138131 -> K+0xbd8). A bank is filled by two chunks with
  that NAME: `zaa_` gives its clip set (bank+0x10), `zaf_` its skeleton (bank+8; B1 FUN_004bedf8 B1:582053ff; BF2
  keeps the first). `FinAnimation` (0xef563a98) then looks its value up in the clip set by the engine CRC-32 of the
  string as written, with no prefix (BF1 B1:102412-102420 -> K+0x25fc, FUN_0043d718; BF2 B2:138020-138031 ->
  K+0xbdc), and is ignored when no bank is set yet. [AGENT_B B1q]
- [L] `bankls`: each fin bank holds exactly ONE clip, the nine-pose itself (`rep_fightertank_9pose`,
  `imp_speederbike_9pose`, `barcspeeder_9pose`, `tat_skiff_9pose`, `all_snowspeeder_9pose`), and `zafdump` shows its
  rig holds exactly the clip's bones: the fighter tank's 5 (dummyroot, both skis straight under dummyroot, a winglet
  under each ski), the bike's 10 (dummyroot, bone_root at (0, 0.679, 0), 8 parts under bone_root). [L] In RAM the
  palette's index 0 is dummyroot in both (BF2 E+0x620) and the model's other nodes (the tank's bone_root, turret,
  guns) are not in it.

### 4.2 Constructor and drive
- [V] The constructor binds the palette to the bank's rig and the clip at rate 30 with loop 0, sets frame 4, samples,
  composes and fills the table (BF1 B1:96465-96482; BF2 B2:130936-130949). The numbers a, b, c are set to -1
  (B1:96488-96490 *(decompiler detail omitted)* = E+0x212c..0x2134; B2:130953-130955 E+0x1cb0..0x1cb8); p and y are
  zeroed earlier (B1:96395-96396 *(decompiler detail omitted)*, `[0x1a0]` = E+0x67c / 0x680; B2:130826-130827 E+0x54c / 0x550)
  [V], and read 0 [L] (786 of 786 never-driven hovers, `checks/never_driven_py.txt`). The constructor's fin fill
  runs only when the bank and clip are set.
- [V] The drive, while the hover has a pilot and both class words (BF1 FUN_001ec768 B1:99593-99636; BF2 FUN_0021d8b8
  B2:134693-134740): p and y smoothed at rate 5 (re-vehicle-riders 4.1), a = y + 4, b = y + (p > 0 ? 1 : 7),
  c = |p|; set frame a (`FUN_0043e830`: u = a / (frames - 1), B1:499352-499380; BF2 FUN_00532fc8), advance by 0
  (leaves u), sample with flag 1, set frame b, blend by c (FUN_0043ff58 / FUN_00533920), compose from the identity
  (0x66ef20 / 0x73d8c0, BSS set by a static initialiser; [L] identity in RAM, `checks/roots_ram.txt`), fill the table
  (FUN_00498040 / FUN_0057eaa0). BF2 re-binds the palette to the rig every update.
- [D] Without a pilot nothing poses: a hover once ridden keeps its last fin pose; one never ridden shows frame 4.

### 4.3 Sample, blend, compose [V] (AGENT_B B3q, B4q, B7q)
- Sample (FUN_0043fa98 -> FUN_0043e2c0, BF2 FUN_005336b8 -> FUN_00532c80): f = (frames - 1) u, k = trunc f; rotation
  **slerp** (FUN_00445720 / FUN_0053a3e8, shortest arc) and translation lerp between frame k and k + 1 by f - k
  (when k + 1 reaches the last frame a looping player wraps to frame 1, a non-looping one - the fin player - clamps);
  with flag 1 a rig bone without a track takes its bind.
- Blend (FUN_0043ff58 / FUN_00533920): per bone with a track, q = slerp(q_a, q_b, c), t = t_a + c (t_b - t_a), in
  bone-local space (a pose bone holding the 1e6 'unset' sentinel takes the sample instead; the hover never does).
- Two tracks for one bone: the bone map is built in track order and the **later** track wins (FUN_00446f88
  B1:506135; BF2 FUN_0053bb10 B2:670197) - the bike clip's second `bone_root` track (the moving one) and the tank's
  second `dummyroot`.
- Compose (FUN_004410e0 / FUN_00440ed0, BF2 FUN_00534338): bone 0's translation is dropped, every other bone is
  mat(q, t) x parent with t the pose's translation when the bone record's byte +0x67 bit 0 is set (the skeleton
  loaders set it on every bone; only soldier code clears it), else the bind's; a non-root `DummyRoot` copies its
  parent.

### 4.4 The clips [L] (`findump`, the decoder `mungefs::anim`)

| clip (class) | frames | bones it moves (range over the 9 frames) |
|---|---|---|
| rep_fightertank_9pose (both fighter tanks, both games) | 9 | bone_tankski_L/R 45 deg, bone_winglet_L/R 95 deg |
| imp_speederbike_9pose (both bikes, both games) | 10 | bone_root 10 deg (pitch +-5) and y 0.659-0.699, handlebars 60, foils 50, rear flaps 100, foot pedals 40 |
| barcspeeder_9pose (BF2 BARC) | 9 | boneroot 9.5, pedals 87, handles 63 |
| tat_skiff_9pose (BF1 skiff) | 9 | bone_rudderL/R 80, bone_leverL/R 17 |
| all_snowspeeder_9pose (both snowspeeders) | 9 | bottom flaps 55, top flaps 52 |

Frame 4 equals the bind in every clip (within 0.04 deg, with the quaternion read as the transposed matrix as the
PS2's FUN_00446110 builds it). No clip has a track on a node any of its class's sections names as a turret, aimer or
barrel node (`nodecensus_<g>.txt` against the clip dumps). The bike's frame 9 is never sampled (a spans 3-5, b 0-2 or
6-8).

### 4.5 The replay [L]
`replay_fin.py <game> <rig> <clip> <tables>` computes the pose of section 4.3 at each vehicle's RAM numbers (a, b,
c) and compares every rig bone with the table entry the PS2 holds (a never-driven vehicle at frame 4):

| population | fits (worst bone < 0.05 deg, 0.5 mm) | worst | rules that fail on the driven rows |
|---|---|---|---|
| BF2 bikes, 6 ride states x 9 bikes | 54 / 54 | 0.001 deg | nearest frame (5.7 deg), floor frame (35 deg), first duplicate track (2.9 deg) |
| BF1 bikes, 12 ride states (player + Endor bots) | 69 / 69 | 0.001 deg | nearest 16.8, floor 36.0, first duplicate 5.0 |
| BF1 fighter tanks, every state | 56 / 56 | 0.001 deg | nearest 7.2, floor 25.0 |
| BF2 fighter tanks, every state | 114 / 114 | 0.001 deg | nearest 11.7, floor 24.7 |
| BF2 BARC | 4 / 4 | 0.001 deg | nearest / floor 10.1 |

nlerp for both steps instead of slerp fits within 0.075 deg (rejected at the 0.05 bar on 7 of 69 BF1 bikes). The
first-duplicate rule can only fail where a clip doubles a MOVING bone - the bikes' bone_root; the tank's and the
BARC's doubled dummyroot is static, so there both rules score the same.

## 5. Riders on posed nodes [O][L]
- [V] The seat function places a rider at `L x E x O` with E = his hardpoint's parent's entry NOW (re-vehicle-riders
  2.1), and the hover tick seats the pilot after the drive's fin fill (2.3).
- [L] The BF2 BARC (`ref/states/bf2_kas2_cw_rep_spawnmap.p2s`, a bot riding, a 4.46 c 0.105): its rider sits at
  (0, 0.0001, 0.0100), up (0, 1.000, -0.010) in the vehicle's frame = `hp_rider`'s bind relative to boneroot
  ((0, -1, 0)) x boneroot's fin entry (Y row (0, 0.9999, -0.0101), at (0, 1, 0)) - he leans with the BARC's body.
- [D] The bikes' `hp_active` hangs from dummyroot, which the clip holds at the identity: a bike rider does not move
  with `bone_root`'s lean (re-vehicle-riders 3's 12 of 12 ride states at the bike's origin agree).
- [L] The fighter tank's gunner rides `turret_y` (re-vehicle-riders 3; section 3 above reproduces his place).

## 6. Walkers, flyers and the hailfire (not this card's build) [V][H]
- [V] Walkers and flyers own a table the same way and their draws refresh aimers and turrets into it before drawing (BF1
  walker FUN_00217278 B1:128485/128496/128593, flyer FUN_001d8620 B1:84597/84608/84637; BF2 walker FUN_0025b428
  B2:176748/176954/176971 - the turrets only while its fade alpha is at least 0.8, flyer FUN_00201300
  B2:115458/115473/115481); the walker's skinned draw reads the table per palette bone (AGENT_A A5). [L] The aimer
  formula fits the flyers' aimer nodes in RAM (X-wing, gunship, MAF, TIE fighters, every row) and the snowspeeder's
  tow-cable gun (36/36, its parent `towcablegun_y` not in the table: its bind), but NOT the walkers' (spider
  `turret_y_2` 0/37, AT-TE `aimer_rgun_1` 0/19) nor the AT-TE's main cannon turret node (0/19) nor the MAF's rear cannon
  (0/7), nor BF1's gunship ball turrets off yaw 0 (10/24): their parent nodes are posed by the walk / flyer animation
  and are not in the table where the replay looks - [H], not read.
  **Corrected (card VPOSW, 2026-10-22, `re-walker-nodes.md` [O]):** those misfits were the replay's own misread.
  `ps2table.py` read every vehicle's table at the hover's offset. Read at the walker's own table (BF1 E+0x1ec0, BF2
  E+0x1d7c), every walker node fits: BF1 1,205 / 1,206 rows, BF2 all within 0.04. Read at the flyer's own table
  (BF1 E+0x1ce0, BF2 E+0x1940), all 1,501 flyer node rows fit, the MAF's rear cannon 7/7 and the ball turrets 24/24
  included (`lead_tmp/vposw/ftable.py`). The walk-posed parent is in the table: the constructor registers the walk
  pose's own matrices.
- [V] The flyer's fins (AGENT_B B5q): `AnimationName` also looks up a clip "takeoff" and a clip "fins" (BF1 FUN_001e06c0
  B1:90342-90366, K+0x1770 / 0x1774; BF2 FUN_002120e8, K+0x824 / 0x828; `FinAnimation` replaces the fins clip); the pose
  (BF1 FUN_001d8620, BF2 FUN_00201300) runs while the flyer's bank word (BF1 E+0x62c) is set, and a class with a takeoff
  clip poses it by lift fraction instead of the fins (B1:84512-84519, B2:115236); else a = turn + 4, b = turn + (s > 0 ?
  1 : 7), c = |s|, turn the raw turn axis (BF1 E+0x308, BF2 E+0x2e4), s the speed setpoint placed on MinSpeed / MidSpeed
  / MaxSpeed in [-1, 1] (B1:84525-84557, B2:115336-115384; BF2's getters scale and cap the three speeds while E+0x668 is
  set). [L] The snowspeeder's bank has no takeoff clip, so its flaps pose by this law.
- [V] The hailfire droid's draw re-aims three nodes named by `SuspensionNodeName` / `SuspensionLeftArmNodeName` /
  `SuspensionRightArmNodeName` (FNV 0xe534c2cb / 0xabb24116 / 0x580b459f; BF1 K+0x27a4..0x27ac, B1:96879-96983;
  BF2 B2:131396-131580); only `cis_tread_hailfire` authors them (CIS_HailFire_Droid_center, armL, armR) - on no
  showcase map. The look-at they build is not read [H].

## 7. For the port (card VPOSE build)
What exists: hovers drawn rigid (`viewer/vehicles.rs`); a building turret's per-node parts and node laws
(`bake/turret_rig.rs`, `viewer/turret_pose.rs`: `turret_node`, `aimer_node`, `barrel_node`); a walker's one-bone CPU
skin (`viewer/walker_pose.rs`, `bake/walker.rs` `skin_glb`); the seat guns' look and aim per seat (`core/src/vehicle/
gun.rs` `GunState`); the hover's p and y per vehicle (`soldier_vis::step_hover_nines`, `turret_pose::HoverNine`);
riders hung at `L x bind(parent) x O` (`soldier_vis::rider_now`).
1. **Bake** (rebake=side): per hover class with a posed node or a FinAnimation, its node rig (each `skel` node's bind
   matrix and parent, as `ManifestTurretRig`), the model drawn per node (one-bone skin, or per-node parts for the
   models with no spanning triangle), and the FinAnimation clip with its rig: the 9 (10) frames of every rig bone
   (rotation, translation, the later duplicate track kept), the rig's parents and rests.
2. **Pose** per drawn hover, each tick: every node at its bind; the fin rig's bones from `HoverNine` (a, b, c) by
   section 4.3 (slerp + slerp, compose in the rig, bone 0 without translation, a non-root DummyRoot = its parent),
   only while the hover has a pilot (else the last pose; a fresh hover frame 4 = the bind); each TURRET section's
   turret node from its seat's `look` (yaw, pitch x `PitchTurnFactor`, through its parent's matrix now), its aimer
   node(s) along the seat's `aim`, its barrels as `turret_pose::barrel_node`; each OWN weapon's aimer node from the
   pilot seat's aim (each aimer's own direction is PROVISIONAL: the port has one aim per seat, the PS2 one aimer per
   weapon chasing the same aim point from its own pivot; a direction from the node's bind origin to the seat's aim
   point within the aimer's limits is the PS2's steady state). Visual only: no sim hash.
3. **Riders**: `rider_now` hangs a rider from his hardpoint's parent's POSED matrix (the BARC's boneroot, the
   fighter tank's turret_y), the same tick.
4. **Fix** `HoverNine::START` to p = y = 0 (section 0.6); the nine-pose numbers stay "never driven" until the first
   piloted step.
5. Not in this card: walkers' and flyers' posed nodes (card VPOSW: the AT-ST's head and guns, the AT-AT's head, the
   one-man AT-ST, the AT-TE, the spider; card VPOSF: the gunships' turrets, the snowspeeder's flaps and tow gun), the
   hailfire's suspension.

## 8. Still [H] / not read
- The walkers' and flyers' parent step (section 6): which matrix a walker's aimer or turret node is built on when its
  parent is posed by the walk clip; the gunship ball turrets off yaw 0; the MAF's rear cannon. Settled by card VPOSW
  (`re-walker-nodes.md`): the parent's own table entry, read at the vehicle's own table offset.
- BF1's in-place writes can land in a model's SHARED bind matrices when a node's first entry is the bind pointer
  (AGENT_A A4) - whether any class shows it on screen was not looked for.
- The hailfire's suspension look-at (section 6); the BF1 STAP's and the hovernaut's posed hp nodes draw nothing.
- BF2 writes aimer nodes at draw time only (AGENT_A A2, a range-limited grep, [D]); the RAM's 0.01 residuals agree.
- The constructor pose's untracked-bone translations (AGENT_B B4q): no fin rig has an untracked bone, so it does not
  arise for the five clips.

## 9. Tools and data (`engine/artifacts/lead_tmp/vpose/`)
`nodecensus.py` (+ `nodecensus_<g>.txt`), `skel.py` (a model's raw skel, the engine CRC, bind matrices),
`findump/` (a clip's tracks per bone, `--json`), `zafdump/` (a bank's rig, `--json`), `bankls/` (a bank's clips),
`clips/*.json` (the five fin clips and rigs of both games), `ps2table.py` (every pad vehicle's node table in a save
state, `ps2/<state>_table.json`), `replay_fin.py`, `replay_turret.py` (+ `.txt`), `replay_aimer_veh.py` (+ `.txt`),
`checks/` (fn and word checks, roots, never-driven p and y), `claims/claims.json`, `verify/` (the workflow's
verdicts), the reader reports `AGENT_A.md`, `AGENT_B.md`.

## 10. Checks run and the verifier
**Self-checks** (`lead_tmp/vpose/checks/`): `re_check.py fn` on every cited line (`fn_b1.txt`, `fn_b2.txt`: each in
the function named); `re_check.py callers` for FUN_00498040, FUN_0057eaa0, FUN_0057ea80, FUN_00222f20;
`re_check.py writers` for DAT_00600548 / 549 / 50c; `re_check.py word` for the draw vtable words 0x531524 (0x1f2468)
and 0x61cbf4 (0x224848) and the aimer think 0x539264 (0x26d748) (`words.txt`); the compose roots 0x66ef20 / 0x73d8c0
are BSS, read in RAM (`roots_ram.txt`); absence receipts [receipt:14ce016e] [receipt:450bf128] (the table is
cleared only in constructors) and [receipt:fc46c5f0] (the frame-set / sample / blend trio's callers); the key hashes
of the hailfire's three nodes resolved by FNV against the key dictionaries and their classes counted on both discs.

**Workflow `vpose-re-verify`** (7 agents, effort medium; claims and citations only, `claims/claims.json`,
`verify/verdicts.json`): five first verifiers over 40 claims in five batches (BF1 sampler, BF1 table / turret /
order, BF2 twins, flyer and other draws, the measurements re-run), then two second verifiers over the 15 twin and
absence claims, working separately. First reading: 23 SUPPORTED, 17 PARTLY, 0 NOT SUPPORTED. Second reading: 7
SUPPORTED, 8 PARTLY, 0 NOT SUPPORTED; no claim the first reader supported was overturned (one PARTLY - the BF2 turret
constructor's gates - read SUPPORTED the second time), so none is downgraded to [H]. Every PARTLY is folded in above
in its corrected form. What the verifiers found that this session had not: both constructors ZERO p and y
explicitly (B1:96395-96396, B2:130826-130827) - the claim had said "not written"; the bone-map write of the hover's
player is in FUN_00446f88 itself (B1:506135), FUN_00446d90 serving a larger player type; a non-root bone takes the
pose's translation only through the skeleton's +0x67 flag; the sampler's wrap at the last frame for a looping
player; the blend's 1e6 sentinel; an aimer's task key comes from its CLASS record; the BF2 walker refreshes its
turrets only at fade alpha >= 0.8; the flyer pose's bank gate and BF2's speed getters; the first-duplicate rule only
discriminates on the bikes; BF2 aimer rows within 1e-3 are 75-100 %, not 81-100 %.

**Readers**: AGENT_A (the vehicle draws, aimers, the table's lifecycle, walkers and flyers), AGENT_B (the FinAnimation
setter, constructor, sampler, blend, compose, flyer fins, duplicate tracks). Their "not settled" lists are section 8.
