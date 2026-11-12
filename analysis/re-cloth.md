# OPUS RE (2026-10-16) - [O] UNAUDITED

Load-bearing claims are tagged [O]; the verifier's table is in section 9.

# RE: BF2 cloth - the `CLTH` mesh, who wears it, the cloth step and its draw (card CLOTH)

BF2 hangs **cloth** (a separate textured mesh, part pinned to bones, the rest simulated) on the models whose `gmod`
lists it: the Hoth snowtroopers' skirt, the clone capes, the heroes' capes and robes, the CTF flags. BF1 has none
(`re-chunk-census.md` 4, [receipt:c3674481]). This doc gives what the port needs to parse, place, simulate and draw it.

Hops: the level loader FUN_00177058 (`CLTH` -> FUN_001d9be0, `gmod` -> FUN_001c5ed8) -> the ClothData parser
FUN_001d9ca8 -> the class EntityClothClass (ctor FUN_001da478, SetProperty FUN_001da5b0) -> the soldier spawn
FUN_00234a68 -> the EntityCloth (FUN_001da9a8, ctor FUN_001dc2a0, buffers FUN_001dc3b8) -> the soldier draw
FUN_00239bb0 -> FUN_001de2a8 (step + draw) -> FUN_001dcdd0 / FUN_001dcf90 (FUN_001dd130 forces, FUN_001dd048 Verlet,
FUN_001ddf40 constraints + FUN_001ddad0 collisions, FUN_001de118 normals) -> FUN_003c3520 / FUN_003c30c8 (draw).
Checked on the PS2's RAM (HOT1 save state) and on the disc (all 58 chunks).

Tools (`engine/artifacts/lead_tmp/cloth/tools/`): `clth.py` (every CLTH chunk parsed the way FUN_001d9ca8 reads it;
`--verts`), `ramcloth.py <state.p2s>` (each soldier's gmod object, cloth instance, class values, live vertices),
`clothclasses.py` (every BASE `cloth` class and the ClothODF users), `place.py` (a CLTH on its model's `skel` bind
pose), `mipsdis.py`. Outputs: `clth_all.txt`, `clothclasses.txt`, `checks.txt`, `checks_absent.txt`, `claims.md`,
`verify_result.json`.

## 1. The chunk (`CLTH`) [V][O]

FUN_001d9be0 allocates a 0x90-byte **ClothData** (pool 0x686b00 "ClothData"), parses it with FUN_001d9ca8 and keeps
the first one per NAME in the table 0x686ba8 (a later chunk of the same name - IMP.LVL has three skirts, one per
`lvl_` - is freed) (B2:86539-86562). Children (byte stream, strings NUL-terminated and NOT padded inside DATA / COLL):

| child | read as | ClothData |
|---|---|---|
| `INFO` | the owner model's name (FNV) | [8] = +0x20 |
| `NAME` | the cloth's name (FNV) - the key | [6] = +0x18 |
| `PRNT` | the bone the cloth frame hangs on (CRC, FUN_00539420) | [7] = +0x1c |
| `XFRM` | 12 f32: rows r0 r1 r2 t (row vectors), the cloth frame in the PRNT bone's frame | [0x10..0x1f] |
| `DATA` | texture name (FNV) [0x20]; u32 N; N x vec3 positions; N x vec2 UV (stored x2048 fixed); u32 F; u32 W (0 or F) + W bone names (CRC); u32 T + T x 3 u32 indices; three lists, each u32 count + count x (u32 a, u32 b) | [0]..[5], [9]..[0xf] |
| `COLL` | u32 K; K x (bone name (CRC), u32 type, 3 f32 params, 12 f32 rows) | [0x22], [0x23] (0x60 each) |

(B2:86603-86856.) **The pinned vertices are 0..F-1**: no index list is read; the forces and the integrator run
`for i in F..N` (B2:89036, 89171) and a constraint moves only the free end of a pinned-free pair (B2:89268-89288).
W = 0 (the CTF flags) leaves the pinned vertices in the cloth frame with no bone. FUN_001d97c8 stores every list
entry's rest length |p[a]-p[b]| and the bounding-box diagonal (B2:86353-86535).

The disc (`clth.py`): all 58 chunks parse with 0 bytes left in DATA and COLL, every index < N. The showcase one,
IMP.LVL `imp_inf_snowtrooper_skirt`: N 54, F 18 (all weighted to `bone_pelvis`), T 80, lists 68 / 64 / 64, PRNT
`bone_root`, texture `imp_inf_snowtrooper` (the body's own sheet, v 0.64..1.0), 4 COLL cylinders (type 1) on the thighs
and calves (thigh: radius 0.16, param 0.5; calf 0.13, 0.5). On its model's `skel` bind pose (`place.py`:
v . XFRM . Bind(PRNT)) the pinned ring sits at y 1.105..1.226 m (pelvis 1.159) and the hem at 0.472 m (calf bone
0.581): a skirt from the waist to just below the knee - the PS2 frame `pcsx2/ref/bf2_hot1/cp1_cp6_postA_spawn.png`.

## 2. Who wears it [V][O]

- The `gmod` chunk (the geometry's LOD table) carries the list: FUN_001c59e8 reads INFO = u32, f32, **u32 count
  (8 slots), count cloth names** into +0x54 / +0x58.. of the geometry object (table 0x684530) (B2:72347-72404).
  Correction (cleanse 19, 2026-10-20): the 8 is the size of the slot array, not a check. B2:72386-72393 stores all
  `count` names, and B2:72395-72400 zero-fills the slots from `count` up to 8. A count over 8 runs into the fields
  after the slots. The port's reader returns no list for a count over 8, which is its own choice (`mungefs::entity`).
- At spawn FUN_00234a68 walks that list of the soldier's geometry object (E+0x160): for each name, the class with that
  name (the global class list, +0x14) is instantiated through its vtable +0x10 into **E+0x10a4 + 4i**; a name the
  class lookup does not find leaves its slot 0, nothing drawn (B2:149930-149990). The instance finds its ClothData by the CLASS
  name (B2:88320).
- So a cloth needs three things of one name: the `gmod` INFO entry, a class with `ClassLabel cloth` (its ODF), and the
  `CLTH` chunk. **`ClothODF` is not read by the game**: its key 0xf4d25eda is nowhere in the BF2 ELF or decomp
  [receipt:aa5571e8] - it only makes the munger pack the cloth ODF.
- RAM (`ramcloth.py`, `bf2_hot1_cp1_cp6_postA_spawn.p2s`): all 13 Imperial rifleman / rocketeer / sniper (snow) share
  ONE geometry object (imp_inf_snowtrooper) listing `imp_inf_snowtrooper_skirt`, each with its own EntityCloth
  (vtable 0x6194b8, N 54 F 18 T 80); the Wookiee's lists `all_inf_wookie_dreads4,2,3,1` (the disc gmod's order) with
  null slots - the census of the disc's 35 cloth classes (`clothclasses.py`) has none of those names, so the Wookiee's
  dreads are not drawn; likewise `rep_inf_ep3cloakedanakin_skirt` and `CIS_CountDooku_Cape_Low1`.
- 35 cloth classes on the disc (`clothclasses.txt`); 33 gmods carry a cloth list (`tools/gmodcloth.py`,
  `gmodcloth.txt`). On the showcase battle HOT1 (GCW): the snow rifleman, rocketeer and sniper (gmod imp_inf_snowtrooper:
  the skirt) and Vader (gmod imp_inf_darthvader: the cape, class Priority 6). Off the showcase [D]: NAB2's two lamp-post
  cloth classes are on no gmod list (and have no chunk), so they are not drawn; the Alliance carried CTF flag's gmod lists
  `com_icon_neutral_carried_cloth`, and of INGAME.LVL's two chunks of that name the first (the neutral flag's) is kept.

## 3. The class (EntityClothClass) [V][O]

Ctor FUN_001da478, SetProperty FUN_001da5b0 (B2:86858-87053), ELF defaults 0x5fcb8c..0x5fcba0:

| key (FNV) | field | default | read as |
|---|---|---|---|
| AttachedMesh 0xe90ecacc | +0x60 | - | the model whose skeleton places the pins |
| WindDirection 0x415e2b30 | +0x70 | (1, 0, 0) (FUN_001debf0) | two angles, "%f %f" |
| WindSpeed 0x13901a72 | +0x80 | 10.0 | |
| Dampening 0xc1133964 | +0x84 | 0.5 | share of the frame's acceleration felt |
| Drag 0xb88c0d43 | +0x88 | -2.0 | |
| ParticleMass 0x6eb957f3 | +0x8c | 1.0 | |
| MaxAcceleration 0x4e994b61 | +0x90 | 20.0 | frame acceleration clamp |
| (no key) | +0x94 | 10.0 | frame velocity clamp |
| Priority 0x94e4e309 | +0x98 | 0.0 | "%f": the buffer budget's order |
| CrossConstraint 0xde112858 | bit 32 | on | gates list 3 |
| BendConstraint 0x94dd79b9 | bit 33 | on | gates list 2 |
| StretchConstraint 0x8311cd99 | bit 34 | on | gates list 1 |
| HardEdge 0x5caf15e1 | bit 35 | off | alpha-test GS state |
| Transparent 0xafe34731 | bit 36 | off | blended GS state |

The constraint keys gate the DATA lists in that crossed order (bit 32 -> the 3rd list, 33 -> the 2nd, 34 -> the 1st,
B2:89920-89955); every disc cloth class leaves all three on, so the order of names never shows. The snowtrooper skirt
sets only AttachedMesh (all defaults); NAB2's lamp cloths and Yoda set WindSpeed / Dampening; the heroes set Priority;
Chewbacca's fur sets Transparent. RAM: the skirt class reads wind (1,0,0), 10, 0.5, -2, 1, 20, +0x94 10, flags & 0x1f = 7.

## 4. The step [V][O]

Once per game update FUN_001dc7f0 sets the step's dt = **min(update dt, 0.039)** (RAM: 0.0333) (B2:88488-88506); the
soldier's draw runs the step at most once per update (dt x the time scale FUN_003da598, 1.0 when <= 0,
B2:90116-90121; FUN_001dcf90's frame stamp B2:88985-89010).
The **cloth frame** = entity x pose[PRNT] x XFRM (B2:90122-90200); positions live in that frame. Per step:

1. **Frame motion** (FUN_001dcdd0, B2:88869-88983): v = (t - t_prev)/dt clamped to |v| <= 10 (+0x94); a = (v - v_prev)/dt
   clamped to |a| <= MaxAcceleration.
2. **Forces** (FUN_001dd130, B2:89083-89234), in the cloth frame (R = its rotation): gl = R^T((0, -9.8, 0) - Dampening x a);
   wl = R^T WindDirection; vl = R^T v; one random r in [0, 1) per step (LCG 0x19660d / 0x3c6ef35f). For each free
   vertex i >= F: u = (p_i - old_i)/dt + vl; f = Drag x u + n_i (r x WindSpeed x (n_i . wl)); acc_i = f / ParticleMass + gl.
3. **Verlet** (FUN_001dd048, B2:89012-89081): p_i += (p_i - old_i) + acc_i dt^2; old_i = previous p_i. No damping term
   besides Drag.
4. **Pins** (FUN_001dca78, B2:88634-88867): p_i (i < F) = XFRM^-1 x pose[PRNT]^-1 x pose[bone_i] x local_i, local_i set
   once (FUN_001dc840) = M1[bone_i] x M0[PRNT] x XFRM x rest_i from the AttachedMesh model's skeleton (M0 the bind
   matrices, M1 their inverses [D: the bind pose reproduces the rest positions]) - i.e. each pin is rigidly skinned to
   its bone.
5. **Constraints**, `iterations` times (FUN_001ddf40, B2:89897-89966): list 3, list 2, list 1 (each when its bit is
   set), each entry (a, b, rest): d = p_b - p_a, k = (|d| - rest)/|d|; a < F -> only p_b moves, p_b -= k d (even when
   b < F too - no disc chunk has an entry with both ends pinned); else b < F -> p_a += k d; else each moves half
   (FUN_001dd430, B2:89236-89320; the stiffness 1.0 passed in is never read). Then the **collisions** (FUN_001ddad0,
   while the never-written byte DAT_005fcbb1 = 1 [receipt:a1b7fa17]): each COLL shape's frame = (pose[PRNT] x XFRM)^-1
   x (pose[bone], or the model's bind matrix when the pose lacks the bone) x its rows - FUN_001dcf90 is handed the cloth
   frame without the entity, so this is all in model space; free vertices are pushed out of: type 0 a sphere (radius
   p0, FUN_001dd5a0); type 1 a cylinder about the shape's 2nd row (radius p0, half length **2 x p1**: radial push when
   the radial depth is the smaller, else +row1 x the axial depth, unsigned, FUN_001dd8c0); type 2 a box (half extents
   p0..p2, least-depth axis, signed, FUN_001dd660) (B2:89322-89598).
   `iterations` = 5 when the soldier is a human player's (E+0x330 >= 0) and prone (stance E+0x7e4 == 2), else **1**
   (B2:155302-155338) - and BF2's prone has no entry path (`re-stance-roll.md`), so always 1 in play [D].
6. **Normals** (FUN_001de118, B2:89968-90061): per vertex the sum of its triangles' (p_b - p_a) x (p_c - p_a), normalised.

Initial state: the position buffers +0x20 and +0x24 start as copies of the rest positions (B2:88363-88366), the frame's
velocity at 0.
RAM (`ramcloth.py --verts`, reproduced by the verifier): of the 13 skirts ONE has been stepped - the player's, the one on
screen (+0x84 = 5403): its 36 free vertices sit 0.12-0.36 m from the rest (v48 rest (0.357, -0.003, -0.526), live
(0.165, 0.053, -0.517)) and its pinned rings keep their local z (-1.2257 / -1.1047) turned with the pelvis; the other 11
with buffers were never drawn (+0x84 = 0) and hold the rest positions exactly. So the PS2 simulates the skirt, and only
while it is drawn [V].

## 5. The draw [V][D]

FUN_001de2a8 draws after the step: skipped when the pose is missing and the cloth has bone pins (B2:90104-90110), a
bounding-sphere frustum test (B2:90224), then FUN_003c3520 (B2:420171-420290) with the cloth frame as the model matrix,
the DATA texture, the soldier's tint and fade as the vertex colour, the soldier's lights, and FUN_003c30c8 as the vertex
source: the triangle list once, front order, normals x127, UVs (B2:419997-420169). FUN_003c30c8 also holds a second half
that replays the list in reverse order, but its only caller (FUN_004958a8 through the `addiu` at 0x3c3824, vertex count
3T; the decomp's one reference, the ELF's one `addiu` [receipt:f4a72a3d]) asks for indices below 3T only, so that
half is dead [V, verifier D7]. The cloth is still **two-sided**: the GS never culls, and VU1 culls only for a material
whose x word carries 0x800800 (`re-vu1-cull.md` 2, CPU side [V], its VU1 reading agent-only); the cloth's material
words are 0x4000 (opaque) / 0x4044 (Transparent) - lit, no cull bits [D]. The back faces keep the front normal
(one-sided lighting). GS states (FUN_003c3868, B2:420292-420313) [D, from the GS register layouts]: opaque = Z test on,
no alpha test, PRIM gouraud + textured (0x18); Transparent = alpha blend (Cs-Cd)As+Cd (0x4044), ABE (0x58), alpha test
NEVER / FB_ONLY = no Z write (0x51001); HardEdge = alpha test on (0x5000b). The skirt is opaque, not HardEdge.

## 6. The PS2's limits, and the port's choices

- **Master switch** (byte DAT_005fcbb0, default 1, B2:86071-86077): at level start the code at 0x175bd8-0x175c68 (an
  export hole) turns cloth OFF for the level when the free heap after loading, less 0x20864 bytes, is <= 32 KiB, else on
  with two pools (HOT1 RAM: on, pools 0x20000 + 0x40000). Off = no cloth instance at all [V]. Which BF2 maps fall under
  it is not known [H] - the port draws cloth everywhere (a modern machine has the memory; named improvement).
- **Buffer budget**: each cloth needs 4 x N x 16 + F x 16 bytes from the pool (B2:88340-88367); when it is short, the
  cloth with the lowest (Priority + 1) x max(1 - frames-since-drawn/30, 0.1) loses its buffers and is not drawn
  (B2:88168-88182, FUN_001dc078). The skirt needs 3,744 bytes: about 100 fit in the HOT1 pools, so the budget never
  bites on the showcase [D]. The port keeps no budget.
- The step runs from the draw, so a cloth off screen does not move (RAM: 11 of 13 skirts at rest) [V]; it starts from
  the rest shape the first time it is drawn. The port may step every cloth within view distance each tick (a drawn
  cloth looks the same; a soldier walking into view shows a settled skirt rather than the rest shape for its first
  frames - a named improvement).

## 7. What stays [H]

- That the skeleton arrays at model+0xb0 [0] / [1] are the `skel` chunk's bind matrices and their inverses: the bind
  placement in `place.py` lands the skirt on the waist (1) and the RAM's pinned ring is the rest ring turned (4), but the
  arrays' construction was not read. The build card's picture checks it.
- The pose matrices (the soldier draw's *(decompiler detail omitted)*, a table by bone CRC at +8) are taken as model-space bone matrices.
- The level-start memory rule's per-map outcome; the 6th caller of FUN_001de2a8 (FUN_0022edd8, slots at +0x170, no pose:
  props / flags) was not followed.

## 8. Cards

- **CLOTH** -> phase build (slice 1): parse CLTH in mungefs; bake each soldier's cloths (gmod list x cloth class x CLTH)
  as rigid-skinned geometry in its GLB (pins on their bones, free vertices on the nearest pin's bone, in bind space
  v . XFRM . Bind(PRNT)); drawn two-sided with the DATA texture.
- **CLOTH2** (new, M1, 4, deps CLOTH): the step of 4 on the CPU with a per-frame vertex upload, the PS2's constants.
- **Troop cloth** (2026-11-12): the step reached heroes only, because only the hero bake wrote the physics companion
  (`<model>.cloths.ron`). The soldier bake now writes it for every wearer, as FUN_00234a68 builds a cloth for every
  soldier whose gmod lists one (2). The BF2 troop bodies that wear cloth on the disc are `imp_inf_snowtrooper` (the
  skirt) and `rep_inf_arctrooper`, `rep_inf_clonecommander`, `rep_inf_ep3heavytrooper` and `rep_inf_ep3sniper` (capes).
  No other troop or local body in the bake has a cloth list. Every other baked soldier file is byte-identical to the
  previous bake, and the hero companions too.

## 9. Checks run

`checks.txt` (check 1: fn on 86 cited lines, all in the functions named here; check 4: ELF words 0x5fcb7c..0x5fcba0,
0x5fcbb0 = 0x101, the format strings, both vtables, the pool names, and the code immediates 0x3d1fbe77, 0x3a83126f,
0xc11ccccd, 0x19660d / 0x3c6ef35f, 0x45000000, 0x3d088889, 0xfffdf79c / 0x8000 / 0x20000 / 0x40000; check 5: writers of
DAT_005fcbb0 / bb1 / 00686db0 / db4 / dc0 / 5fcba8 / ac; callers). `checks_absent.txt`: widescan of 0x5fcbb0 (two stores:
0x1d90d8, 0x1d9608) and 0x5fcbb1 (one load), constscan + widescan + re_check absent of 0xf4d25eda [receipt:aa5571e8],
`DAT_005fcbb1 =` [receipt:a1b7fa17], jal scans of 0x1d90d0 / 0x1d90c0 / 0x1d90e0 / 0x1da9a8 / 0x1de2a8.
Verifier (workflow `cloth-re-verify`, 8 agents, effort medium; `verify_result.json`): batches A-E (35 claims) 34
SUPPORTED, 1 PARTLY (D7: the reverse half of FUN_003c30c8 is out of its one caller's range - section 5 rewritten,
two-sidedness now from the material words); the absence / twin set X (6 claims, each read by two separate verifiers
with their own ELF scans) 12 of 12 SUPPORTED - the second reader overturned nothing; the measurement agent reproduced
M1, M2, M4, M5, M6 and returned M3 PARTLY (only the drawn skirt is stepped - folded into 4 and 6 as a finding; it also
found `place.py`'s pinned/free label bug, fixed). Folded-in details: A4's asymmetric pin rule, C1's model-space frame
for the collisions, the pools' ELF defaults 0x8000 + 0x8000 before the level start sets them.
