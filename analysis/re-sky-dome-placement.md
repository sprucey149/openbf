# RE: sky-dome placement — how the PS2 positions, scales and blends `DomeInfo.DomeModel` shells (2026-09-03, Fable, analysis only)

Corpora: `analysis/SWBFRNT2.ELF-decomp.c` (BF2), `analysis/SWBFRONT.ELF-decomp.c` (BF1),
the raw ELFs `extracted/bf{1,2}/SWBF*.ELF` (PT_LOAD reader `elfrd.py` in the session
scratchpad `re_sky/`, used for vtables and `.data` initial values), one Ghidra headless
listing of BF2 `0x3cbb00..0x3cbc10` (`re_sky/disas_3cbb.txt`, `DisasAt.java`), the raw
`extracted/bf2/lvl/DATA/_LVL_PS2/MUS/MUS1.LVL` (sky_ records, `segm MTRL` words) and the
port's baked `bf2_mus1/models/mus1_sky_volcano.glb` (ring radii). Nothing under `engine/`,
PLAN, HANDOFF or RE_LOG was touched; no cargo run.

Tags: [V] verified in the decomp / ELF bytes / disc bytes, [D] derived, [H] hypothesis.

Hash preimages used (fnv1a-32 of the lowercased key, the `PROP_` convention of
`engine/crates/game/src/layout.rs`, checked against `PROP_TEAM` = fnv1a("team") = 0xA2FD7D0C):
`DomeModel` 0x82681057, `Geometry` 0x2a6d8591, `Offset` 0x14c8d3ca, `MovementScale` 0xfd58fbfa,
`RotationSpeed` 0x32048fba, `Effect` 0x6e6e8d54, **`Reflect` 0x92c778aa (new preimage)**,
**`Stars` 0x622b0026 (new preimage)**, `NearSceneRange` 0x50551c9a, `FarSceneRange` 0x01192b7b,
`MaxDistance` 0xe6915cf6, `PatchResolution` 0x6ef738fb.

## 0. Headline

1. **Where the properties are consumed.** BF2: `FUN_003bcd50` (sky_ loader, decomp 416116–416600)
   parses a `DomeModel` scope at line 416396 into a 0x400-byte slot per shell (index at
   sky+0x38, max 10): `Geometry` → model pointer at +0x40 (416438–416441, model table
   `0x76c310`), `Offset` → f32 at +0x44 (416434–416436), `MovementScale` → f32 at +0x48
   (416497 → 416587–416588), and EITHER of those sets the flag +0x4c = 1 (416589–416590);
   also `RotationSpeed` → +0x54 speed + normalised axis at +0x60 (416418–416437), `Reflect` →
   byte +0x50 (416465–416467), `Effect` → up to 20 hard-point effects at +0x70 (416473–416496).
   BF1: `FUN_002e29e0` (269983–271000) at 270092, 0x14-byte slots from +0x24: `Geometry`
   +0x24 (270138–270141), `Offset` +0x28 (270146–270149), `MovementScale` +0x2c (270159–270161),
   flag +0x30 (270148), `Reflect` +0x34 (270154). No `RotationSpeed`/`Effect` in BF1. [V]
   Defaults from the constructors: Offset 0, MovementScale **1.0**, flag 0
   (BF2 `FUN_003bcb08` 416054–416066; BF1 `FUN_002e2888` 269931–269939). [V]
2. **`Offset` is applied per frame, in the sky tick, along WORLD Y only, in the same units as
   the camera position (world metres), and only through the lerp**
   `pos.y = cam.y + ((base + Offset) − cam.y) · (1 − MovementScale) · S`, with `base` = 0
   (BF2 sky+0x2840, written only by the ctor, 416071 [V]; BF1 +0xec, ctor 269940 [V], no
   other writer found [D]) and `S` = the camera scale field (= 1.0, see 4). X and Z are the
   camera's, untouched. BF2 `FUN_003cb918` 425830–425837, BF1 `FUN_002f2308` 280211–280221. [V]
   So with S = 1 the shell origin is `y = MS · cam.y + (1 − MS) · Offset`: **MovementScale = 1
   pins the shell to the eye, MovementScale = 0 pins it at world height `Offset`.** `Offset`
   is NOT added on top of a camera-locked shell — the port's `eye·MS + (0, Offset, 0)` is
   the wrong reading; the ELF's Offset contribution is `Offset · (1 − MS)` (0.5 m for
   Mustafar's 100 / 0.995). Sign: +Y up, same as the camera. Nothing is baked once.
3. **`MovementScale` multiplies nothing but that Y lerp.** It does not scale X/Z (the port's
   `eye * movement_scale` horizontal lag is not in the ELF). 1.0 = locked to the eye. [V]
4. **The dome mesh is drawn at its AUTHORED size, times the camera scale `S` = `cam+0x140`
   (BF2) / `cam+0x90` (BF1), and `S` is 1.0.** BF2: model matrix = Rotation(RotationSpeed·t)
   × Scale(S,S,S) with row 3 = pos (425872–425884; `FUN_0053b0a8` 669795 is a pure diagonal
   scale [V]); the rotate+scale block is gated on `DAT_005ffd78`, whose `.data` byte is 01
   [V, ELF bytes] — the listing `re_sky/disas_3cbb.txt` shows `lbu a2,-0x288(a2)` / `beq
   a2,zero` skipping both `jal 0x53ab30` and `jal 0x53b0a8` [V]. BF1 scales unconditionally
   (280273). `cam+0x140` is written only as 1.0 (`FUN_00391680` region 389111/389251/389357/
   389476; every other `+0x140` store in the decomp is a different struct) [V]; it is the
   distance multiplier that `FUN_0057b850` (711660) and the far-scene pass also use. So on the
   PS2 `mus1_sky_dome` is a ~20 m shell around the eye and `mus1_sky_volcano`'s rings are at
   radius 18.1 / 19.0 m. The sky pass is the far-scene pass: the tick queues the shells in
   layer 0x20000 and immediately flushes `FUN_005b06b8(…, 0x60000, 0)` (426027) whose range is
   `[NearSceneRange[1] · S, FarSceneRange]` = MUS1 `[80, 2000]` (defaults 72 / 2000,
   `FUN_00564f50` 698725, `FUN_00565070` 698760, disc values from the raw MUS1 sky_) [V for
   the numbers, H that this is the GS depth range rather than only the cull range]. Depth is
   NOT off: the GS TEST word is 0x50000 (ZTE=1, ZTST=GEQUAL, Z write) for an opaque segment and
   0x51001 (ATST=NEVER, AFAIL=FB_ONLY = no Z write) for a blended one (`FUN_0058ed70` 723257,
   selection at 723411–723474) [V]; the sky is the first thing drawn each frame (task
   priority −9999, 416043 / 269927 [V]) so the test is moot in practice [D].
5. **Blend is per MATERIAL, not per pass.** For a draw with the sky flags 0x20031 (unlit 0x10,
   no distance fade 0x20000, "sky" 0x20 → no Z write when blended, `FUN_0058ed70`
   723411–723474 / `FUN_0057cf60` 712624): opaque unless (colour alpha ≠ 0xff) or MTRL bit
   0x04 (transparent → ALPHA 0x44 = Cs·As + Cd·(1−As)) or bit 0x40 (→ 0x48 additive); bit
   0x02 = alpha test (TEST 0x5000b + global AREF `DAT_0060c64c` = 0x60). The dome colour is
   `sky+0x2855`, default `DAT_0076c200` = (ff,ff,ff,ff) (`FUN_0057ca38` 712404–712407) [V].
   **LEAD CORRECTION (2026-09-03, from `mungefs::model`'s parse — the tested parser — and the
   baked GLB `extras.flags`):** the raw-offset read below mixed the two models up.
   `mus1_sky_dome` has TWO segments, MTRL 0x00010001 and 0x00010010 (no bit 0x02/0x04 → drawn
   opaque, Z-written, un-blended, un-tested), and `mus1_sky_volcano` has ONE segment, MTRL
   **0x00010003 — bit 0x02 set → alpha-TESTED** on the PS2 (TEST 0x5000b, AREF 0x60 [D]): its
   transparent top rows are cut and its opaque skirt (alpha 252 at v = 1.0) is painted — so the
   upper band's bottom edge IS drawn by the PS2, at the elevation §7 gives (below the horizon).

   **LEAD CORRECTION 2 (2026-09-03, session 12, `analysis/re-sky-pass.md` §3 and `re-material-flags.md` §1, lead
   re-read of 723449–723480):** on the SKY path the draw flag 0x20 (always set by the sky tick, 0x20031) changes the
   rule: a segment with bit 0x02 takes the TRANSLUCENT path (ALPHA 0x44, TEST 0x51001) — it is BLENDED by its texture
   alpha, not alpha-tested; the AREF word 0x5000b is reached only on the non-sky path. And every sky draw, opaque
   included, takes TEST 0x51001 = Z-test GEQUAL with NO Z write. So `mus1_sky_volcano` (0x10003) composites on the
   PS2; the "alpha-TESTED, AREF 0x60" reading above is wrong for sky draws. BF1's Normal-class sky variant
   (`FUN_004d0108` 594636–594648) blends AND tests a 0x02 shell at AREF 0x40, but no BF1 sky shell carries 0x02.
   Wrap
   mode: **CLOSED 2026-09-03 (session 12, `analysis/re-texture-wrap.md`): GLOBAL REPEAT/REPEAT for every model
   draw in both games — CLAMP_1 comes from a texture-record qword the loader zeroes unconditionally; the PS2
   samples Hoth's v = 1.001 ring wrapped too, so the port's V clamp is a stated deviation.** (Original text:)
   not closed — the draw path builds its GS packets from prebuilt tables
   (`DAT_007c57b0..7c5848`, 724937–724965) plus the texture's own word (`FUN_00498b30`,
   723945); no CLAMP register write was found in the code traced, so [H] the wrap mode is
   carried inside the munged texture/segment packets. The port samples REPEAT
   (`render/src/texture.rs:100`).
6. **The seam is a port defect, not the disc's.** With the ELF rule (S = 1, mesh at authored
   size), `mus1_sky_volcano`'s upper band bottom edge (mesh y −1.78 at ring radius 19.04) sits at
   `atan2(−1.78 + (100 − cam.y)·0.005, 19.04)` = **−3.9° at cam.y = 0, −4.9° at cam.y = 68 m**
   (MUS1's walkable ground: hint nodes at y ≈ 68), −5.3° at 100 m. The port puts it at
   **+0.6°** (lead's measurement reproduced by the same arithmetic: +0.61° at 68 m). On the PS2
   the edge is ~5° below the horizon; a chase camera 2 m over a soldier sees below −4.9° only
   where the ground drops ≥ 0.085 × distance (a 25 m drop at 300 m), which MUS1's walled lava
   valley with the LOWR skirt does not offer from the play area [D]. The lower band's bottom
   (y −3.69, r 18.10) is at −11°. What the ELF supports for the port: translation =
   `eye + (0, (Offset − eye.y)·(1 − MS)·k, 0)`, scale `k` (any k, uniform — angles are
   invariant), no X/Z lag, optional yaw = RotationSpeed·t; and the volcano shell opaque.

## 1. Loader — BF2 `FUN_003bcd50` (416116–416600) [V]

The sky_ scope is walked with `FUN_00535e78` (record) / `FUN_00535f40` (child scope).
`DomeModel` (0x82681057) at 416396 opens a child scope and fills slot
`sky + 0x40 + n·0x400` where `n = *(sky+0x38)`; the slot count is incremented at scope end
(416410). Per record:

| hash | key | store | lines |
|---|---|---|---|
| 0x2a6d8591 | Geometry | `FUN_00539150(0x76c310, 0x800, hash)` → model ptr at +0x40 | 416438–416441 |
| 0x14c8d3ca | Offset | f32 → +0x44, then `+0x4c = 1` | 416434–416436, 416589–416590 |
| 0xfd58fbfa | MovementScale | f32 → +0x48, then `+0x4c = 1` | 416497, 416587–416590 |
| 0x32048fba | RotationSpeed | f32 → +0x54; args 2–4 normalised (VU `vrsqrt`) → axis at +0x60 | 416418–416437 |
| 0x92c778aa | Reflect | byte +0x50 = 1 | 416465–416467 |
| 0x6e6e8d54 | Effect | (name, hp, min, max) ×≤20 at +0x70 stride 0x30, count +0x430 | 416473–416496 |

Constructor `FUN_003bcb08` (416020): each slot +0x40 = 0, +0x44 = 0, **+0x48 = 1.0**, +0x4c = 0,
+0x50 = 0, +0x54 = 0, +0x64 = 1.0 (axis default (0,1,0)), +0x430 = 0 (416054–416066);
`+0x2840 = 0` (416071, the only writer of that word); registers the object in the tick list
with priority −9999 (*(decompiler detail omitted)* 416043 → `FUN_005652f0` 698920,
ascending insertion, so it runs first). The vtable at +0x18 is `DAT_00632750`; its slot +0x18
(the tick, invoked by `FUN_00563a20` 697953 as `vt[+0x18](obj, 0)`) is **`FUN_003cb918`**
(ELF bytes at 0x00632768 = 0x003cb918) [V].

Other sky_ scopes in the same function, for orientation: `LowResTerrain` 0xddf0c29e →
`Texture` +0x2844, `MaxDistance` +0x2848 (default 2000), `PatchResolution` +0x284c (5),
0x6d9b436e +0x2850 (1.0) (416248–416283); `Stars` 0x622b0026 → a 0x16c sub-object at +0x285c
parsed by `FUN_003bfc30` (416521–416533), drawn by `FUN_003c10a8(…, 0x20031)`.

## 2. Loader — BF1 `FUN_002e29e0` (269983) [V]

Same shape, 0x14-byte slots from `sky+0x24`, index at +0x20: `Geometry` +0x24 (270138–270141,
model table `0x69c678`), `Offset` +0x28 (270146–270149, sets +0x30 = 1), `MovementScale` +0x2c
(270159–270161, sets +0x30 = 1), `Reflect` +0x34 (270154). Constructor `FUN_002e2888`
(269913): ten slots {0, 0, **1.0**, 0, 0} (269931–269939), +0xec = 0 (269940), priority −9999
(269927). Tick = vtable `DAT_00541588` slot +0x18 = **`FUN_002f2308`** (ELF bytes at
0x005415a0) [V].

## 3. Per-frame placement — BF2 `FUN_003cb918` (425708–426030) [V]

*(decompiler excerpt omitted from the public copy)*

`FUN_0053b0a8` (669795) multiplies each matrix row by diag(sx, sy, sz, 1) — a pure scale [V].
`FUN_0053ab30` (669375) builds an axis-angle rotation from `cos/sin` (`FUN_0053a070`) [V].
The Ghidra listing (`re_sky/disas_3cbb.txt`) confirms the gate and that the scale uses
`f20` (= S loaded at 425784): `003cbb1c lbu a2,-0x288(a2)` (0x5ffd78), `003cbb3c beq a2,zero,
0x003cbbc0`, `003cbba4 jal 0x0053ab30`, `003cbbb8 jal 0x0053b0a8` with `f12..f14 = f20`, then
`003cbbc0 lq v0,0x40(sp); sq v0,0xb0(sp)` (position into row 3) and `swc1 f0,0x3c(s4)` (w = 1)
[V]. `DAT_005ffd78` = 0x01 in `.data` (elfrd; the ELF has one PT_LOAD 0x160000, filesz
0x5012c8, so 0x5ffd78 is initialised) [V]. `DAT_005ffd80` = 2.0 and `DAT_005ffd84` = 0.006 are
the time accumulator (425805) and sun-flicker constants, unrelated. **CORRECTED 2026-09-29 (card REFL,
`analysis/re-sky-reflect.md` §6/§7 [O]):** wrong — the accumulator is `DAT_005ffd7c` (425799), 0x5ffd80 is its rate
(+2.0/30 per tick) and 0x5ffd84 the amplitude of the Reflect draws' Y scale −(0.85 + 0.006·sin φ); 425805 is a matrix copy.

**S = 1.0.** `cam+0x140` is stored only as 0x3f800000 (four camera slots of 0x150 bytes,
`FUN_00391680` region 389111 / 389251 / 389357 / 389476) [V]; the grep of every `+0x140) =`
in the decomp shows no other float writer (the rest are unrelated structs: counters, bitmasks,
pointers) [V]. `FUN_0057b850` (711660) reads it as the divisor of the camera's half-extents,
and `FUN_005b06b8` (746670) multiplies the scene near/far by it — it is a world-distance
multiplier that stays 1.0 on the disc [D].

`FUN_00589e38(0)` / restore: `DAT_0060c2a8` (init 1 at 715920) is read by `FUN_00589e48` in
`FUN_005b0…` at 727615 to choose `FUN_00584a10` vs `FUN_00584a90` on `DAT_00661728` — [H] the
fog/lighting-table enable, cleared while the sky is queued.

## 4. Per-frame placement — BF1 `FUN_002f2308` (280127–280305) [V]

Identical structure: `cam = FUN_0049c3d0()` (`DAT_00522ac8`), `S = cam+0x90` (280180),
`pos = cam+0x30`, `pos.y = cam.y + ((sky+0xec + Offset) − cam.y) · (1.0 − MovementScale) · S`
(280211–280221), Reflect draw at +0.1 y with 0x1000031 then 0x20031 (280243–280255),
`M = identity · Scale(S,S,S)` (`FUN_00446590` 505728, same diagonal scale — unconditional,
280273), `M.row3 = pos` (280274–280277), shell 0 drawn with `sky+0x100` colour, others with
`0x69c420`, flags 0x20031 (280279–280286), then `FUN_00481300(0x20000)` /
`FUN_004c6608(DAT_00522fa8, 0x60000, 0)` (280298–280302). BF1 has no rotation. `cam+0x90`:
written 1.0 at 51960 / 52237 / 263784 / 553439; the other `+0x90` writers in the grep are
different structs [D — the BF1 camera class was not walked as carefully as BF2's].

## 5. The draw path and what 0x20031 means — BF2 [V]

`draw` is `RedModel` vtable slot +0xc (`*(model+0xc0)` = `DAT_0064feb0`, ELF bytes: slot 3 =
`FUN_0057cf60`, RTTI string "8RedModel" follows the table) [V].

`FUN_0057cf60` (712624): 0x10 is forced on when `FUN_0057f8e8()` == 0; with 0x10 set the
lighting block is skipped (`(flags & 0x80000010) == 0` false) → **unlit**; the per-segment
sphere cull uses the far-scene far (`FUN_00565140` = `DAT_00746024`) when 0x20000 is set,
else −1 (712728–712732); 0x10 forced at 712760; the alpha distance fade is skipped when `(flags & 0x4020000) != 0`
(712818–712828) → **sky shells are never distance-faded**; then `FUN_0058ed70(segment, M, 0, 0,
flags, colour, …)` per segment.

`FUN_0058ed70` (723257): with `f` = segment+0x18 material flags and `c` = colour:
- early out if `c.a == 0` (723386);
- `translucent = (c.a != 0xff) || (f & 4) || (flags & 0x800000)` (723411);
- bucket (*(decompiler detail omitted)*): 0x20000 → 8, or 9 if translucent and `(flags & 0x20) == 0` (723436–723438);
- GS ALPHA (*(decompiler detail omitted)*): translucent → 0x44 (Cs·As + Cd·(1−As)); `f & 0x40` or 0x800000 → 0x48
  (additive); `f & 8` → 0x64 with FIX = alpha/2 (723450–723460);
- GS TEST (*(decompiler detail omitted)*): translucent → 0x51001 (ATE, ATST NEVER, AFAIL FB_ONLY: no Z write);
  `f & 2` → 0x5000b (ATST GEQUAL, AREF = `DAT_0060c64c` << 4 = 0x60); else 0x50000 (ZTE,
  ZTST GEQUAL) (723465–723480, AREF at 723553 / 723598);
- 0x10 clears the lighting bit 0x4000 in the prim word (723477–723480).

**MUS1's shells — CORRECTED by the lead (see §0/§5 note):** `mus1_sky_dome` = two segments,
flags 0x00010001 and 0x00010010, texture `mus1_sky_dome`; `mus1_sky_volcano` = one segment,
flags 0x00010003, texture `mus1_sky_volcano` (`mungefs::model`, and the baked GLB extras).
The raw-offset read that follows was wrong about which `modl` was which: dome → opaque,
TEST 0x50000; volcano → alpha-tested, TEST 0x5000b with AREF 0x60 [D: the MTRL→segment+0x18
copy (724209–724225) not followed]. **SUPERSEDED by §0 LEAD CORRECTION 2 (and RE_LOG 2026-09-03): a segment with
bit 0x02 takes the TRANSLUCENT path (ALPHA 0x44, TEST 0x51001), so `mus1_sky_volcano` (0x00010003) COMPOSITES —
it is blended, not alpha-tested; the AREF word is reached only on the non-sky path.** (Original text: "`mus1_sky_dome` one segment 0x00010001
textured `dea1_bldg_corridor_prison`; `mus1_sky_volcano` two segments 0x00010001/0x00010010"
— a misread.)
The port's `bake::detail::TRANSPARENT` = 0x04 / `CUTOUT` = 0x02 already encode the same two
bits, so its material verdict for these shells is "opaque" too; the port's SKY PIPELINE,
however, always alpha-composites (`render/src/lib.rs:684–724`, `BlendState::ALPHA_BLENDING`,
depth Always/no write) — that is where the transparent skirt of the volcano becomes a hole.

Colour tables: `DAT_0076c200..0x76c25f` is a 4-byte-stride colour table initialised in
`FUN_0057ca38` (712404–712440): entry 0 = (ff,ff,ff,ff) white, entry 1 (0x76c220) =
(0,0,0,ff) black, entry 2 (0x76c228) = (200,100,100,ff) … `sky+0x2855` is the per-frame lerp
white→black by `obj+0x38` in `FUN_0045ae80` (520150–520300) — [H] a damage/lightning flash
or fade; its default is entry 0. The `SkyInfo Color(…)` records go to a different table
(`PTR_DAT_0060c2f0[i]`, `FUN_00589930` called at 416336) with default alpha 0x20 when only three
args are given.

## 6. The sky pass range — BF2 [V numbers / H depth semantics]

`FUN_005b06b8(queue, 0x60000, 0)` (746670): for a flush with bit 0x40000 set, `near =
DAT_0074601c · S` (S = `cam+0x140` when `DAT_00746028`), `far = DAT_00746024`, returns if
`far < near`, then `FUN_005ae940(near, far, 0x7ec470, cam)` (frustum record: stores near/far
at [0x21]/[0x20] and builds the planes with `FUN_0057b850`'s half-extents, plus a 7× wider set)
[V]. `DAT_0074601c` / `DAT_00746020` / `DAT_00746024` default 72 / 120 / 2000 (698058–698061)
and are set by the sky_ keys: `NearSceneRange(a, b, c)` → `FUN_003be0e0` → `FUN_00564f50`
(698725): `DAT_00746018 = a` (clamped by a Z-precision formula), `DAT_0074601c = b` (unless
the debug override `DAT_0060c120`), `…
`FarSceneRange(d)` → `FUN_003be100` → `FUN_00565070` (698760): `DAT_00746024 = d`, far-scene
enable = d > 0 [V]. MUS1 authors `NearSceneRange(0.5, 80, 110)`, `FarSceneRange(2000)` (raw
sky_ at 0x563f1) [V]. So the sky is flushed with the far scene, range [80, 2000] on MUS1,
while the dome mesh itself is 18–20 m from the eye — i.e. the shell lies in front of that
pass's near distance. [H] the far-scene "near" is a cull/overlap distance and the GS depth
range for the far pass starts at the real near plane (`DAT_00746018`) — the GS does not
near-clip, `FUN_0057cf60`'s sphere test passes a sphere centred on the eye, and the disc
visibly draws the domes. **Not closed**: where the far-scene pass's Z buffer is set up /
cleared relative to the near-scene pass.

## 7. Where the PS2 puts `mus1_sky_volcano` — derived [D]

Ring geometry from the baked GLB (identical to the authored mesh up to the port's
dequantisation): band A rings y −3.69 / +3.69 at r 18.10 (v 1.0 / 0.024), band B rings
y −1.78 / +5.98 at r 19.04 (v 1.0 / 0.031). Offset 100, MovementScale 0.995 → Y displacement of
the shell relative to the eye `dy = (100 − cam.y) · 0.005` (S = 1).

| edge | cam.y = 0 | cam.y = 68 (MUS1 ground) | cam.y = 100 | port today |
|---|---|---|---|---|
| band B bottom (v = 1, the seam) | −3.85° | **−4.86°** | −5.34° | **+0.61°** |
| band A bottom (v = 1) | −10.0° | −11.0° | −11.5° | −5.4° |
| band A top | +13.0° | +12.0° | +11.5° | +17.4° |
| band B top | +18.8° | +17.9° | +17.4° | +22.7° |

(`atan2(y + dy, r)`; the port column is `atan2(y·k + 100 − 0.005·cam.y, r·k)` with
k = 1000/19.9 — reproduces the lead's +0.6°.) The port's error is the sum of two things: it
adds the full `Offset` (100 m over a 1000 m shell = +5.7°) where the ELF adds `Offset·(1−MS)`
(0.5 m over a 19 m shell = +1.5°), and it does not scale the offset with the shell. The
volcano bands therefore sit ~4.3–4.5° higher in the port than on the disc, which is exactly
what lifts the opaque v = 1 edge from −4.9° (hidden behind MUS1's terrain skyline for any
ground/chase camera; there are no flyers on MUS1) to +0.6° (open sky).

A PS2 player camera can see −4.9° of sky only where the terrain skyline drops below that —
0.085 × distance of relief — which MUS1's enclosed valley plus the `LowResTerrain`
(`MaxDistance` 2000 default, own fog) does not provide from the play area. **Verdict: the seam
is the port's.** [D]

## 8. What the port should do (derived)

1. `Renderer::sky_model` (`render/src/lib.rs:926`): translation `eye + (0, (offset − eye.y) ·
   (1 − movement_scale) · k, 0)` where `k` is the shell's uniform scale; drop the `eye ·
   movement_scale` horizontal lag. With `k = SKY_SHELL_RADIUS / radius` the ELF's angles are
   reproduced exactly, since a uniform scale about the eye is angle-invariant. (Keeping
   `SKY_SHELL_RADIUS` = 1000 is fine; the ELF's own S is 1 and the mesh is ~20 m, but the port
   draws the sky with depth off so the radius only has to clear the near plane.)
2. Optional, cheap: yaw = `rotation_speed · t · 2π` about the authored axis for shells that
   carry `RotationSpeed` (BF2 only, 0 on MUS1; `DAT_00661318` = 1/frame-rate-ish, 667388).
3. Sky shells whose MTRL word has neither 0x02 nor 0x04 should be drawn OPAQUE (no alpha
   composite), which is what the disc does with `mus1_sky_volcano` — the transparent skirt is
   then dark texels below the horizon, not a hole; shells with 0x04 blend with 0x44 (standard
   over), 0x02 alpha-test at AREF 0x60/255.
4. `Reflect` shells are drawn twice (once with the 0x2000000 "reflection" bit at +0.1 m) —
   ignore until a map needs it (none of the 32 baked maps has been checked for `Reflect`).
   **SUPERSEDED 2026-09-29 (card REFL):** BF1 KAS1 and KAS2 author it; both mirrored draws are read and measured on the
   PS2 — §11 below and `analysis/re-sky-reflect.md` [O].

## 9. Negatives / not closed

- Texture wrap (CLAMP vs REPEAT) for dome segments: not found in the traced draw code; the GS
  packets come from prebuilt tables plus the texture's own word — [H] baked in the munged
  `tex_`/segm packets. Open.
- The far-scene pass's depth setup (whether Z is cleared between the near and far passes, and
  the actual GS Z range for the sky): open; only the cull/frustum record was traced.
- The loader copy of the MTRL flag word into segment+0x18 was read at its first use but the
  store was not followed (724209–724225).
- The BF1 draw path (`RedModel` vtable) was not traced; the BF1 tick uses the same 0x20031
  flags and is assumed [H] to reach an equivalent of `FUN_0058ed70`.
- `sky+0x2855` lerp (`FUN_0045ae80`) — which game event drives it is unknown.
- 0xe1bca028 (scope with `Color(…)` entries feeding `FUN_00589930`) and 0x9010bf4a /
  0xeeeb3a51 / 0x3ce6a053 in the sky_ loader: preimages not found.
- MUS1 camera heights were taken from hint nodes (y ≈ 68); the CP platforms / lava floor were
  not measured — the −4.9° figure moves by ±0.5° over cam.y 0..150.

## 10. Reproduction

```
# property hashes (fnv1a-32, lowercased)
python3 -c "h=lambda s:__import__('functools').reduce(lambda a,c:((a^c)*0x01000193)&0xffffffff,s.lower().encode(),0x811c9dc5);print(hex(h('Offset')),hex(h('MovementScale')),hex(h('Reflect')),hex(h('Stars')))"
# loader / tick lines
grep -n "0x82681057\|0x14c8d3ca\|0xfd58fbfa" analysis/SWBFRNT2.ELF-decomp.c analysis/SWBFRONT.ELF-decomp.c
sed -n 425825,425884p analysis/SWBFRNT2.ELF-decomp.c     # y lerp, rotation gate, scale, row3
sed -n 280205,280277p analysis/SWBFRONT.ELF-decomp.c     # BF1 equivalent
# ELF bytes: vtable slot, DAT_005ffd78, camera scale default
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 632750 8      # slot +0x18 = 003cb918
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 5ffd78 1      # 00000001
# Ghidra listing of the gate (JAVA_HOME=tools/jdk-21.0.12.1+1)
tools/ghidra_12.1.3_PUBLIC/support/analyzeHeadless ghidra-proj bf2 -process -noanalysis -readOnly \
  -scriptPath re_sky -postScript DisasAt.java out.txt 3cbb00 3cbc10
# MUS1 MTRL words and sky_ ranges: raw scan in re_sky (search b'mus1_sky_volcano', walk segm/MTRL)
```

## 11. `Reflect` — OPUS RE 2026-09-29 (card REFL, lane 2) - [O] UNAUDITED

Full RE, checks and verifier: `analysis/re-sky-reflect.md`. One foreground Opus verifier ran on 18 claims: 14 SUPPORTED,
3 with a caveat, 1 PARTLY, 0 NOT SUPPORTED, all folded in there.
- **The mirror** [V][O]. For every slot whose `Reflect` word is set (B1 +0x34, 280242; B2 byte +0x50, 425841), the tick
  draws the whole shell twice BEFORE its normal draw, with row 1 = −(0.85 + 0.006·sin φ)·Y and row 3 = the shell's
  lerped position + (0, 0.1, 0). The colour is `ReflectionColor` (DomeInfo, fnv 0x8a81de8b, BF1 sky+0x1c, BF2
  sky+0x34, default white).
  - Instruction words: 0.85 = `lui 0x3f59 / ori 0x999a` at 0x2f23d0; 0.1 = 0x3dcccccd; the `jalr` calls at 0x2f24f4
    and 0x2f251c.
  - The phase: φ = `DAT_00518b1c` += `DAT_00518b20` (2.0)·(1/30) per tick, measured 2.0 rad/s on the PS2.
- **The flags** [V][O]. BF1 0x1000031 (B2 0x2000031) sends the first copy to segment bucket 3. That is the MAIN
  target, kicked before the terrain that shares the bucket, with no Z write. The second copy (0x20031) goes to bucket
  9, the FAR target, in submission order ahead of the normal shell.
  - On BF1 KAS1's lake the PS2 shows the first copy under the Z-written near water, and the second copy past it.
  - NOP-ing each `jalr` in a save state removes exactly one of the two (`re-sky-reflect.md` §4).
- **Maps**: BF1 KAS1 (+ `ReflectionColor(200,200,255)`), KAS2, KAS2_H. None on BF2. §8.4 above is superseded;
  §3's "sun-flicker constants" line is corrected there.
