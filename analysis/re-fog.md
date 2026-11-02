# LEAD VERIFICATION (Fable lead, 2026-09-03, session 12): re-read before acceptance — BF2 `FUN_00580500` 715498–715535 (`near < far` → the fog rows and `… BF1: the hash census (`0x23b1fce` at 270546 only, inside the LowResTerrain scope; `0xcf49a8d4` / `0xfaaa0e17` / `0xd6c706f2` absent from the BF1 decomp), the alpha stretch 270621–270623, the segment gate 594586–594594 (`(flags & 0x20) == 0` && `FUN_004a8090`), `Enable` 271237–271245 (bit 0 → `FUN_002e2810`); `fogspread` appears 12 times in each stringrefs file; no `= 0x3d` A+D write in the BF2 renderer range; the raw HOT1 sky_ records on both discs and BF1 KAS2/KAM1 dumped with the agent's `skydump.py` match §5/§6. Tags and text below are the RE agent's.

# RE: what ENABLES fog on the PS2, with which numbers, on which draws (BF1 + BF2)

2026-09-03, Fable, analysis only. Corpora: `analysis/SWBFRNT2.ELF-decomp.c` (BF2), `analysis/SWBFRONT.ELF-decomp.c`
(BF1), the raw ELFs via `elfrd.py`, the raw map LVLs (all 39 map LVLs on both discs, sky_ records read
byte-wise with `fog_scratch/skydump.py`). No Ghidra run was needed. Nothing under engine/ (other than this
directory), PLAN, HANDOFF, analysis/ touched; no cargo. Tags: [V] verified in decomp/ELF/disc bytes, [D] derived,
[H] hypothesis. Line numbers are decomp lines; BF2 unless marked BF1.

## 0. Headline

1. **Fog on the PS2 is not GS hardware fog.** Neither renderer writes the GS FOGCOL register (A+D reg 0x3d) and
   nothing sets PRIM.FGE. Fog is a SECOND VU1 PROGRAM, literally named `"fogspread"`, appended to a draw batch: it
   redraws the geometry with the fog colour as its colour constant and per-vertex alpha `f = (z - near)/(far - near)`
   from a "fog matrix" (View x [scale,offset] rows), blended with the ALPHA-0x144 family (Cd = lerp(Cd, fog, f)). [V]
   **Corrected 2026-09-29 (card REDB, `analysis/re-fog-amount.md` §2–§5, [O]):** `"fogspread"` is a TEXTURE, not a VU
   program. Every fog pass binds fnv1a("fogspread") = 0x6182a9d0 as its second texture (B1 549414–549415, 594618,
   551406–551407; B2 706800–706801), looked up in the global texture table. It is a 64×32 white texture in `CORE.LVL`
   with alpha 1→248. The fog matrix gives the pass the texture coordinates (s, t) = (f, f), with f from PLANAR view depth,
   and the amount is that texture's alpha, not f. The GS lerps the stored 8-bit values. The PS2's frames put the port's
   linear-light fog ~20 levels too red on BF1 KAS2.
2. **BF2 rule:** fog ON iff `SkyInfo.FogRange(near, far)` is authored with **`near < far`** (`FUN_00580500` 715503 sets
   `… nothing else does). Colour = `SkyInfo.FogColor` RGB, **4th component ignored**. No placeholder
   filter is needed: black is legal (KAM1, TAT3), `0..0` is off (GEO1, KAS2, POL1). Applies to **far-scene draws only**
   (world models, terrain, low-res terrain in layer 0x20000, i.e. beyond `NearSceneRange[2]`); **never** to near-scene
   draws and **never** to sky shells / the base dome (flag 0x20 excludes them). Maps with `FarSceneRange(0)` (END1,
   FEL1, YAV1, TAT3) therefore render NO fog colour at all on the PS2 — only the near->far depth fade `b..c`. [V]
3. **BF1 rule:** `SkyInfo.FogColor` / `FogRange` / `FogRamp` are **not parsed by the BF1 ELF at all** — hash 0x023b1fce
   occurs only inside the `LowResTerrain` scope, 0xcf49a8d4 and 0xfaaa0e17 nowhere. That is the "placeholder filter":
   the ten `FogRange(0, 2700..30000)` maps are dead data. The WORLD fog is `DomeInfo.LowResTerrain.FogColor/FogNear/
   FogFar` (same global setters as the console commands `setfognear/setfogfar/setfogcolor`), with
   **`far' = near + (far - near) * 255 / alpha` when alpha is 1..254** (270621–270623); BF2 KAM1's authored
   `400..1356` is exactly BF1 KAM1's `400..1000, a=160` through that formula. Applies to every opaque model segment
   in BOTH scenes (no layer test in `FUN_004d0108`), terrain and the low-res sheet in the far scene only, never sky. [V]
   **Contested 2026-09-28 (§9.3, [O]):** on the PS2 itself, BF1 KAS2's near models (bamboo at 10–40 m, the soldier) took
   NO fog under a patched 0..200 m ramp; only the far water did. The code reading above stands, but some gate it does
   not name skips them. Card FOGB.
4. **Hoth:** BF1 HOT1 and BF2 HOT1 both fog with (203,201,203), 100..1000 m, linear; BF2 on far-scene draws beyond
   the 200..270 m fade, BF1 additionally on near-scene models from 100 m. `hoth_sky_dome` is fogged on neither. [V]
5. Previous [H] refuted: `DAT_0060c2a8`/`FUN_00589e38` selects the quad-batch program variant (9 = far-scene, 0xc =
   near-scene) in `FUN_00595020`; it is not a fog or lighting enable. `Enable` (BF1) = bit 0 of the value creates the
   sky object (1559 & 1 = 1); BF2 ignores it. `WorldFogRange` is stored and never read. [V]

## 1. STORES

### BF2 — `FUN_003bcd50` (416116) walks sky_; `SkyInfo` records go to `FUN_003be120` (416856) [V]

| key | hash | lines | store |
|---|---|---|---|
| `FogColor` | 0x023b1fce | 416892–416930 | r,g,b clamped 0..255 to bytes, **4th byte forced 0xff** in the word passed (416916–416929 build a second word with the real alpha, but `FUN_00565170` 698830 → `FUN_005804b0` 715474 stores only *(decompiler detail omitted)*) → `DAT_007bb7f0` |
| `FogRange` | 0xcf49a8d4 | 416992–416993 | `FUN_005651b0(near, far)` 698850: `DAT_00746050/54`; `FUN_00580500` 715500: **only if `near < far`**: `DAT_007bb820 = 1/(far-near)`, `DAT_007bb830 = -near/(far-near)` (rows of the fog matrix block `DAT_007bb800..83c`, w-column from `DAT_0073d8c0`), **`DAT_007c5764 = 1`**, `DAT_007c575c/60 = near/far` |
| `WorldFogRange` | 0xd6c706f2 | 416996–416999 | `FUN_005651d8` 698862 → `DAT_00746058/5c`; no reader anywhere (grep) |
| `Enable` | 0xaf8bb8ce | 416983 | `return` — ignored |
| 0xc15d0860 | ? | 416987 | `FUN_00565150` → `DAT_00746028` byte (init 1, 698062); read at 746040/746090/746118 to pick the camera scale for the far scene — not fog |
| `FarSceneRange` | 0x01192b7b | 416942–416950 | `FUN_00565070` 698760: `DAT_00746024`, `DAT_0060c305 = (0 < far)` |
| `NearSceneRange` | 0x50551c9a | 416964–416966 | `FUN_00564f50` 698725 → `FUN_0058ae10` 721338 (depth LUT, §2.5) |
| 0x3c84cf36 / 0x46a20597 / 0x24700fa1 | ? | 416955–416980 | same as `NearSceneRange`, gated on `DAT_00600b30 >= 2` — the split-screen `NearSceneRange` (see 0x23e10337) |
| 0x23e10337 | ? (sub-scope) | 416934–416938 | `FUN_003be658` 417072 re-dispatches its records to `FUN_003be120` **only when `DAT_00600b30 > 1`**. `DAT_00600b30` is 1 from `.data` (`DAT_005f90d8` = 1, 23693), 2 when the split-screen config key 0xb30ae02f is present (14222) or the script value at 42805–42814; BF1's twin `DAT_00519078` is set by `ScriptCB_SetSplitscreen` (19346). So KAS2 `FogRange(5,140)` / MYG1 `FogRange(5,200)` in this scope are the **split-screen fog** [V BF1 name, D BF2] |

`LowResTerrain` in BF2 (416248–416283): `Texture` +0x2844, `MaxDistance` +0x2848, `PatchResolution` +0x284c,
0x6d9b436e +0x2850; at scope end `FUN_00572fe0`/`FUN_00572ff8`/`FUN_0055db38` (707195–707211, 694472) store them
to `DAT_0076c068/64/60` and `DAT_0060bedd`. **No fog keys are handled in BF2's `LowResTerrain`.** [V]

Reset: `FUN_00580d30` 715879 zeroes `DAT_007c575c/60/64` (715913–715915); called from `FUN_00175258` (17147),
itself called from `FUN_001759e0` 17204 and `FUN_00175cd8` 17262 (engine init/shutdown paths). No per-level reset was
found — see §8. Colour default: `DAT_007bb7f0` is .bss = 0 (black) [V, elfrd].

### BF1 — `FUN_002e29e0` (269983) [V]

`SkyInfo` records go to `FUN_002e44d8` (271000–271330). It handles `AmbientColor` 0x627ebb4a, `FarSceneRange`
0x01192b7b → `FUN_00479cf0` 540944, `NearSceneRange` 0x50551c9a → `FUN_00479c10` 540916, 0x3c84cf36 (split-screen
NSR, `DAT_00519078 > 1`, 271105–271107), `VehicleAmbientColor` 0x47c6806c, `CharacterAmbientColor` 0xcda12257,
0xbe437671, 0x91e9c28b → `FUN_00479e90` (a no-op, 541045), 0xf03ee75f, and **`Enable` 0xaf8bb8ce (271237–271245):
`FUN_003947c8(value) & 1` → `FUN_002e2810()` (269887) allocates the 0x108-byte sky object `DAT_00564670`.** 1559 is
odd, 1 is odd: both create the object. `Enable` is the sky-object switch, not a fog switch.

**`FogColor` 0x023b1fce is compared exactly once in the whole BF1 decomp — at 270546, inside the `LowResTerrain`
scope. `FogRange` 0xcf49a8d4, `FogRamp` 0xfaaa0e17 and `WorldFogRange` 0xd6c706f2 are compared nowhere in BF1
(and `FogRamp` nowhere in BF2).** So BF1's `SkyInfo.FogColor/FogRange/FogRamp` are dead data on the disc. [V]

`LowResTerrain` (branch at 270346, records 270546–270618, scope end 270620–270626):

| key | lines | store |
|---|---|---|
| `FogColor` | 270546–270581 | r,g,b clamped; word `(r,g,b,0xff)` → `FUN_00479dc0` 540999 → `FUN_00499798` 560139 → **`DAT_006ea970`**; the 4th arg is kept in *(decompiler detail omitted)* (270566–270575; 0xff when absent) |
| `FogNear` | 270598–270600 | `FUN_00479de0` → `FUN_00499678` 560087: `DAT_006ef3dc = near`, rows `DAT_006ea9a4 = 1/(far-near)`, `DAT_006ea9b4 = -near/(far-near)` in block `DAT_006ea980..9bc` |
| `FogFar` | 270604–270607 | `FUN_00479e00` → `FUN_00499708` 560113: `DAT_006ef3e0 = far`, same rows |
| scope end | 270621–270623 | **`if ((alpha - 1) & 0xff) < 0xfe`, i.e. alpha in 1..254: `FUN_00479e00(near + (far - near) * (255 / alpha))`** — the 4th component is a density that stretches the far distance; 0 and 255 leave `FogFar` as authored |
| scope end | 270624–270625 | `FUN_00488b90(+0xfc, texture)` → `DAT_0069c2c8/c4`, `FUN_00488ba8(MaxDistance)` → `DAT_0069c2c0` (549843–549859) |

The same three setters are the console commands `setfognear` / `setfogfar` / `setfogcolor` (`FUN_002e3de0`;
`FUN_002e26a8` 269831, `FUN_002e2728` 269852, 269881) [V]. There is **no `near < far` test and no enable flag** in
BF1: fog is "on" whenever the rows are non-zero; they are .bss zero at boot (elfrd `0x6ef3dc`, `0x6ea980`) and no
code resets them (writers of `DAT_006ef3dc/e0`: only 560101/560127; the renderer init `FUN_0049a740` 560755 does not
touch them) [V] — consequence in §8.

## 2. CONSUMERS — the mechanism

### 2.1 The fog matrix block [V]

BF2 `FUN_00582248` (716525, once per frame per camera from the scene render 698521 and the mirror/reflection render
`FUN_003c9258` 423934): `M = View(cam+0x70, with y,z negated by FUN_0053b0a8(1,-1,-1)) x FogRows(DAT_007bb800/810/820/830)`
(716589–716640), written by `FUN_00498be0(0, 0x7bb5f0, M, 0, 0x8000)` (716645) into the 0x50-byte "matrix block"
`DAT_007bb5f0`, whose only getter is `FUN_00589c60` (720575). Result: `out.x = out.y = (z_view - near)/(far - near)`,
`out.z = -w`, `out.w = w` [D from the row layout]. BF1 twin `FUN_0049b9a8` 561181 → `0x6ea8b0` (561301), getter
`FUN_004a84a0` 566781. With fog off the rows are zero → f = 0 → the fogspread pass is invisible.
**Amended 2026-09-25 (`re-water-flags.md` §5, `[O]`, verifier-found):** the rows 0x7bb800–0x7bb83f have no writer or
clearer besides `FUN_00580500`, which writes only when near < far, so "zero" is guaranteed for the first map after boot;
rows left by an earlier fog-on map are not ruled out [H]. The port has no session order and keeps the fresh-boot reading.

### 2.2 `"fogspread"` — the fog pass [V]

Strings: BF2 `0x64fc88` (`FUN_00572458`), `0x64fd60` (`FUN_00575670`, `FUN_00575fd8`), `0x650028` (hashed into
`DAT_007c5798` by `FUN_00591a30` 724937–724940); BF1 `0x558d98` (`FUN_00488268`), `0x558e20` (`FUN_0048b9c0`,
`FUN_0048b280`), `0x55e348` (→ `DAT_007015c0`, `FUN_004d0a08` 594763). A batch is a list of up to four VU1 programs
(`FUN_00584c60(mode, p0..p3)` 723967), each with its own matrix block (`FUN_00497ab0` 724010) and colour constant
(`FUN_00497cf0` 724013). The fog pass = program `fogspread` + matrix block `0x7bb5f0` + colour
`FUN_005804d0()` (the fog colour) x `DAT_007c5754` (`FUN_0057fd50`, 1.0; 0.5 via `FUN_00456098` 516761–516764) +
GS ALPHA/TEST words from the table `DAT_007c5810`/`DAT_007c5820` (0x144 / 0x53601 / 0x5d601, built 724941–724965).
Sites — the exhaustive caller lists of the colour copier `FUN_005804d0` (520075, 706814, 708442, 708857, 723668) and
the matrix getter `FUN_00589c60` (706811, 708486, 708889, 723666) are the complete set of fog consumers in BF2:

| BF2 site | what | gate |
|---|---|---|
| `FUN_0058ed70` 723621–723677 (model segment draw, RedModel vtable slot 3 `FUN_0057cf60` → 712870/712920/712952) | adds `fogspread` as program slot 1/2, mode 9 (723634) | **`(flags & 0x20020) == 0x20000`** (far-scene layer AND not sky) && `DAT_0060c2c7` (`FUN_00589870`) && **`DAT_007c5764`** (`FUN_005805b8` 723622) |
| `FUN_00575fd8` 708590 (terrain) far pass *(decompiler detail omitted)* 708405–708460, and the animated layer 708831–708860 | `FUN_00585380` = `… no fog-flag test (zero rows) |
| `FUN_00575670` 708279 (alternate terrain path, `DAT_00661490`) | same | same |
| `FUN_00572458` 706715 (**low-res terrain**, object `0x76c010`: +0x50 = `DAT_0076c060` MaxDistance, +0x54 = `DAT_0076c064` texture; driver `FUN_00571bd0` 706418 from `FUN_0056b9a0` 703098) | `FUN_005849f0` mode 8 plain (706772–706795) or `FUN_00585310` = mode 8 + fogspread (706800–706852) | drawn only when `(flags & 0x20000)` (706497–706499); fogspread only when `FUN_005805b8()` (706771) |
| `FUN_0045ae80` 519989 (520075/520170/520313) | weather/lightning effect: saves, brightens, restores the fog colour | — |

**Corrected 2026-09-23 (cleanse):** these two BF2 functions are the OCEAN and the
FLAT-water renderers (`re-water.md` §3b, verifier-checked); only the BF1 row was relabelled before.
**Amended 2026-09-25 (`re-water-flags.md` §1, `[O]`):** "no fog-flag test" is true of the fog-on flag `DAT_007c5764`, but
the flat renderer's far sub-pass (708831) sits under `SpecularEnable` (`DAT_006615f9`), which defaults to 1 — so it runs.

Near-scene draws — model modes 5 (default 723381) / 0xc / 0xb / 0xe / 3 / 0x12 (723414–723443), terrain mode 7
(`FUN_00584a70` 708716) — receive neither the fog matrix nor the fog colour. **In BF2 the near scene is never
fogged.** [V]

| BF1 site | what | gate |
|---|---|---|
| `FUN_004d0108` 594510 (segment draw = vtable `0x55e334`+0xc, called for every segment by the RedModel draw `FUN_00496248` 557311/557362/557396, layer bits untouched) | `FUN_0049eb90(tex, fogspread)` = `FUN_0049e4b8(9,…)` 594618; skinned variant `FUN_004b1688(…, 0x701580, …)` 594596–594613; colour 594723–594745, matrix 594723 | **`(flags & 0x20) == 0`** && **`DAT_00522b54`** (`FUN_004a8090` 594590) && opaque (*(decompiler detail omitted)*, `seg+0x2c` not 1/2, no 0x400000). **No layer test → near AND far scene** |
| `FUN_0048b9c0` 551186 / `FUN_0048b280` 550926 water low-res pass (WMAP; gated on DAT_005648dc — analysis/re-lowr.md, session 14) | `FUN_0049ec00(layer, fogspread)` 551404–551444 / 551035–551095 | far pass only (`FUN_0048b1e0` 550903) |
| `FUN_00488268` 549331 (**low-res terrain**, object `0x69c270`: +0x50 = `DAT_0069c2c0` MaxDistance, driver `FUN_00487ad8` 549068, 545762) | always `FUN_0049eb90(tex, fogspread)` 549408–549415 | `0 < MaxDistance \|\| +0x64 < 1` |

`DAT_00522b54` is not a fog flag: set 1 at boot (`FUN_00173a98` 16091, param at 16218–16221), cleared around the
sky-object draw `FUN_002e5d60` (271978–271996) and the reflection render `FUN_002ef438` (277977–277995), and it
gates the near/far composite (565466). It is the twin of BF2's `DAT_0060c2c7` (`FUN_00589860`: 16819; sky objects
`FUN_003bf060` 417525–417540 draw with `flags | 0x20001` under it cleared; `FUN_003c9258` 423930–423948). [V]

### 2.3 No GS hardware fog [V]

`grep '0x3d[,;)]'` over the renderer ranges (BF2 690000–760000, BF1 540000–620000) finds no A+D write of register
0x3d; the four `= 0x3d` hits (BF2 767773–768297, BF1 613854–614378) are byte-wise packet builders in the SIF/IOP
region. PRIM words seen in the draw paths (0x46, 0x156, 0x144-family ALPHA, 0x5xxxx TEST) never carry FGE.

### 2.4 Which draw types, and the sky [V]

Sky shells and the base dome are drawn with 0x20031 (BF2 425997–426010, BF1 280279–280286). Bit 0x20 excludes
them from the fog pass in both games (BF2 723621 `(flags & 0x20020) == 0x20000`; BF1 594590 `(flags & 0x20) == 0`).
The `Stars` sub-object and sky-object models (`| 0x20001`) go the same way. Quad batches (`FUN_00595020` 727313:
sprites/particles) pick program mode 9 when `DAT_0060c2a8 == 0` (sky tick, 425786) else 0xc (727614–727621): mode
9 is the far-scene variant (far models 8/9 at 723436–723438, far terrain 9 at 708714, low-res 8), mode 0xc the
near-scene blended variant (723420) — **not a fog enable, and not a fog/lighting table**; `FUN_00584a10`/`a90` are
just `FUN_00585580(tex, mode 9 / 0xc)` (717798–717841). Fog on quads is not decided on the CPU (no fogspread there).

### 2.5 Near scene / far scene — what "far-scene draws" means [V code, D look]

`NearSceneRange(a, b, c)` → `FUN_00564f50` → `FUN_0058ae10` (721338) converts b and c to Z-buffer high bytes
(`DAT_0060c2a0/2a4`) and builds a 256-entry palette `FUN_00584838` (717727): alpha 0x80 for depths beyond c, a ramp
c..b, alpha 0 nearer than b, on a 16x16 8-bit texture named 0x3de761c8 (`FUN_00593d20` 721380). The composite pass
719296–719420 (gated by `DAT_0060c2c7`) draws a full-screen textured sprite (PRIM 0x156, TEST 0x51001, ALPHA 0x48
additive when `DAT_0060c2c8`, which is 1 from 16820/416045) over the near scene using the far-scene buffer blocks
(0x7000/0x7200). `FarSceneRange(0)` → `FUN_00565070(0)`: far cull distance 0 (`FUN_00565140`, used by the model
sphere cull at 712728 and terrain 708407/708418) and `DAT_0060c305 = 0` (skips `FUN_00581a98` 716295). So on
FarSceneRange-0 maps nothing is drawn in the far layer, the near scene ends at c (FEL1: 52 m, END1: 150 m) and fades
b..c into whatever the far buffer holds (the sky) — the fog colour never appears. BF1 has the identical structure
(`FUN_004a57e0` 565466–565560, `FUN_004a97d0` 567625, `FUN_0049e030`).

## 3. THE ENABLE

**BF2:** `near < far` in `FUN_00580500` (715503) and nothing else. `Enable` ignored (416983). Alpha ignored.
FogColor without FogRange (DAG1, MUS1) → colour stored, flag stays 0 → off. `FogRange(0,0)` → off. Black colour
with a valid range (KAM1 400..1356, TAT3 20..40) → **on, black**. [V]

**BF1:** the presence of `LowResTerrain.FogNear`+`FogFar` (14 of 16 maps; RHN1 and RHN2 have no LowResTerrain
scope). The SkyInfo placeholders are filtered by not being parsed. The colour's 4th component is the far-stretch
density (§1). [V]

## 4. BF1 — which field feeds the world, and the [H] on `DAT_0060c2a8`

`LowResTerrain.FogColor/FogNear/FogFar` write the ONE global fog (`DAT_006ea970`, `DAT_006ea980..9bc`,
`DAT_006ef3dc/e0`) that every BF1 fogspread site reads (§2.2). The low-res sheet's `MaxDistance`/texture are separate
globals (`DAT_0069c2c0/c4/c8`) read only by the sheet renderer. So the field is the world fog, applied to models in
both scenes, terrain in the far scene, and the sheet. [V] The `DAT_0060c2a8` [H] is refuted (§2.4). [V]

Cross-check of the alpha formula against BF2's authored data: BF1 KAM1 `FogNear 400, FogFar 1000, a=160` →
`400 + 600·255/160 = 1356.25`; BF2 KAM1 authors `FogRange(400, 1356)`. BF1 HOT1/END1/YAV1 (a=255 or absent) are
byte-identical to BF2 as the README already found. [V, disc bytes]
**Confirmed 2026-09-28 (§9.3, [O]):** the formula holds live. PS2 RAM holds KAM1's far as 1356.25, BES2's 972.5, GEO1's
1096.88, KAS1's 4750, NAB1's 1297.66, YAV2's 1196.88 and KAS2's 2992.1875.

## 5. HOTH — the definitive answer

Raw sky_ bytes (`skydump.py`): BF1 HOT1 `SkyInfo FogColor(128,128,128) FogRange(0,3000) Enable(1) FogRamp(3)
NearSceneRange(1,200,270) FarSceneRange(800)`; `LowResTerrain Texture('hoth') PatchResolution(6) FogNear(100)
FogFar(1000) FogColor(203,201,203,255)`. BF2 HOT1 `Enable(1) FogColor(203,201,203) FogRange(100,1000)
NearSceneRange(0.5,200,270) 0x3c84cf36(1,140,200) FarSceneRange(800)`; `LowResTerrain Texture('hoth')
PatchResolution(6)`. [V]

- **BF1 HOT1:** fog ON, colour (203,201,203), linear 100..1000 m (a=255 → no stretch), on every opaque model segment
  in near and far scene (from 100 m; ≤19 % inside the 200..270 m fade band), on terrain and the low-res sheet only in
  the far scene (beyond ~270 m, culled at 800 → 78 % at the far cull). The `SkyInfo` 128,128,128 / 0..3000 is dead.
- **BF2 HOT1:** fog ON, same colour and range, on far-scene models/terrain/low-res sheet only (i.e. beyond the
  200..270 m near fade, culled at 800); nothing in the near scene.
- **`hoth_sky_dome` and the shells: not fogged on either** (0x20031 → bit 0x20).
- The two Hoths therefore match in colour and ramp; the port rule that makes them match is "fog = LowResTerrain
  triple (BF1) / SkyInfo pair (BF2), linear, excluding the sky".

## 6. Per-map table (disc bytes through the ELF rule; `fog_scratch/fogtable.py`)

`near..far` is what the fog matrix gets (BF1 after the alpha stretch). `b..c` is the near-scene fade band and the
lower bound of "far-scene draws" in BF2. `FarSceneRange 0` = no far layer at all.

| game | map | fog | colour | near..far | b..c | FarSceneRange | note |
|---|---|---|---|---|---|---|---|
| BF1 | BES1 | ON | 82,45,25 | 80..430 | 200..500 | 2000 | SkyInfo 255,0,0/0..2700 dead |
| BF1 | BES2 | ON | 45,45,60 | 80..972 (430, a=100) | 120..180 | 350 | SkyInfo 128/250..350 dead |
| BF1 | END1 | ON | 144,153,156 | 90..350 | 100..150 | 0 | no SkyInfo fog; no far layer |
| BF1 | GEO1 | ON | 142,82,38 | 300..1097 (700, a=128) | 210..280 | 3000 | SkyInfo 252,252,252 dead |
| BF1 | HOT1 | ON | 203,201,203 | 100..1000 | 200..270 | 800 | |
| BF1 | KAM1 | ON | 0,0,0 | 400..1356 (1000, a=160) | 60..90 | 800 | = BF2's 400..1356 |
| BF1 | KAS1 | ON | 0,0,0 | 500..4750 (1000, a=30) | 150..180 | 1000 | ≤11 % black at the cull |
| BF1 | KAS2 | ON | 255,0,0 | 1000..2992 (2000, a=128) | 80..100 | 1500 | **CONFIRMED 2026-09-28 (§9, [O]): disc bytes + live PS2 RAM; no pose shows it — the port's red band is its clear colour** |
| BF1 | NAB1 | ON | 126,148,82 | 700..1298 (1000, a=128) | 100..130 | 1000 | |
| BF1 | NAB2 | ON | 70,70,70 | 3000..6000 (a=0) | 100..150 | 3000 | 0 % inside the 3000 cull |
| BF1 | RHN1 | off | – | – | 100..150 | 100 | no LowResTerrain; see §8 |
| BF1 | RHN2 | off | – | – | 90..110 | 0 | no LowResTerrain; see §8 |
| BF1 | TAT1 | ON | 164,174,194 | 400..10281 (3500, a=80) | 100..150 | 2000 | ≤16 % at the cull |
| BF1 | TAT2 | ON | 164,174,194 | 200..3500 | 150..180 | 3000 | |
| BF1 | YAV1 | ON | 144,153,156 | 90..350 | 50..100 | 0 | no far layer |
| BF1 | YAV2 | ON | 80,80,80 | 400..1197 (800, a=128) | 100..150 | 350 | |
| BF2 | COR1 | off | – | – | 150..200 | 1000 | |
| BF2 | DAG1 | off | (95,111,126) | – | 20..60 | 0 | no FogRange |
| BF2 | DEA1 | off | – | – | 120..180 | 0 | |
| BF2 | END1 | ON | 144,153,156 | 90..350 | 90..150 | 0 | **no far layer → no fog drawn** |
| BF2 | FEL1 | ON | 80,100,80 | -10..62 | 28..52 | 0 | no far layer → no fog drawn; alpha 128 ignored |
| BF2 | GEO1 | off | 0,0,0 | 0..0 | 180..200 | 500 | |
| BF2 | HOT1 | ON | 203,201,203 | 100..1000 | 200..270 | 800 | |
| BF2 | KAM1 | ON | 0,0,0 | 400..1356 | 60..90 | 800 | black is real |
| BF2 | KAS2 | off | 104,106,108 | 0..0 | 85..100 | 5000 | split-screen only: 5..140 |
| BF2 | MUS1 | off | (198,12,8) | – | 80..110 | 2000 | no FogRange |
| BF2 | MYG1 | ON | 194,197,199 | 0.5..600 | 50..80 | 300 | split-screen: 5..200 |
| BF2 | NAB2 | off | – | – | 100..150 | 0 | |
| BF2 | POL1 | off | 180,180,200 | 0..0 | 170..200 | 1000 | |
| BF2 | TAN1 | off | – | – | 60..110 | 150 | |
| BF2 | TAT2 | off | – | – | 70..100 | 3000 | |
| BF2 | TAT3 | ON | 0,0,0 | 20..40 | 20..60 | 0 | no far layer → no fog drawn |
| BF2 | UTA1 | off | – | – | 40..55 | 400 | |
| BF2 | YAV1 | ON | 144,153,156 | 90..350 | 50..100 | 0 | no far layer → no fog drawn |

**Corrected 2026-09-23 (cleanse):** the far scene's terrain is the
`LOWR` sheet drawn out to `MaxDistance` (`re-lowr.md` §2/§4; KAS2 2000 m), not the heightfield ending at the cull,
so the "≤ 25 % red" bound does not hold — this is queue KASF's premise.
**Answered 2026-09-28 (card KASF, §9, [O]):** the row is exact (disc 0x18f368, and PS2 RAM `DAT_006ea970` = 0xff0000ff
with far 2992.1875 in all 8 KAS2 states). All 15 BF1 maps with a PCSX2 state hold their row live. The sheet's bound is
f ≤ 0.50 at MaxDistance 2000, but no captured pose shows a tint (a fog-colour swap on the natural ramp moves nothing
beyond reload noise). The port's pure-red band is its fog-coloured CLEAR colour (card CLRB).

Survivors of the port's `fog_from` (BF1 BES2, KAM1, YAV1, YAV2; BF2 END1, FEL1, HOT1, MYG1, YAV1) vs the ELF: BF1
gains BES1, END1, GEO1, HOT1, KAS1, KAS2, NAB1, NAB2, TAT1, TAT2 with the LowResTerrain numbers and loses nothing;
BF2 gains KAM1 and TAT3 (black). What the port should do with "fog on but no far layer" (BF2 END1/FEL1/YAV1/TAT3)
is a rendering decision: the disc shows a depth fade to the sky between b and c instead of the colour.
**Corrected 2026-09-23 (cleanse):**
implemented as `bake::lighting::fog_from` (1201a92); the rendering decision was taken by queue 6.

### What the bake can implement in place of `fog_from` [D]

```
BF2: if FogRange.near < FogRange.far  → fog { color: FogColor.rgb, near, far }   (alpha ignored, black allowed)
     else none.   Split-screen scope 0x23e10337 may carry its own FogRange (KAS2, MYG1) — ignore for single player.
BF1: if LowResTerrain.FogNear && FogFar → a = FogColor.a (255 if absent);
        far' = a in 1..254 ? near + (far-near)*255/a : far
        fog { color: LowResTerrain.FogColor.rgb, near: FogNear, far: far' }
     else none.   SkyInfo.FogColor/FogRange/FogRamp/Enable: never read.
Both: linear f = clamp((z_view - near)/(far - near)); apply to world models, terrain, low-res sheet; never to
      DomeModel shells, the base dome, Stars or SkyObjects. PS2-faithful extra (optional): BF2 fogs nothing nearer
      than NearSceneRange[2]; BF1 fogs terrain only beyond it.
```

## 7. Reproduction

```
cd <repo>
# stores: hash sites
grep -n -i "0x23b1fce\|0xcf49a8d4\|0xfaaa0e17\|0xaf8bb8ce\|0xd6c706f2\|0xcc60f1a1\|0x7e28d534" analysis/SWBFRNT2.ELF-decomp.c analysis/SWBFRONT.ELF-decomp.c
sed -n 416856,417010p analysis/SWBFRNT2.ELF-decomp.c     # BF2 SkyInfo handler
sed -n 715498,715535p analysis/SWBFRNT2.ELF-decomp.c     # FUN_00580500: near<far → DAT_007c5764
sed -n 270520,270640p analysis/SWBFRONT.ELF-decomp.c     # BF1 LowResTerrain fog + alpha stretch
sed -n 271000,271330p analysis/SWBFRONT.ELF-decomp.c     # BF1 SkyInfo handler (no FogColor/FogRange)
# consumers
grep -n "FUN_005804d0(\|FUN_00589c60(\|FUN_005805b8(" analysis/SWBFRNT2.ELF-decomp.c
grep -n "FUN_004997b8(\|FUN_004a84a0(\|FUN_004a8090(" analysis/SWBFRONT.ELF-decomp.c
sed -n 723610,723680p analysis/SWBFRNT2.ELF-decomp.c; sed -n 594585,594625p analysis/SWBFRONT.ELF-decomp.c
grep -i fogspread analysis/SWBFRNT2.ELF-stringrefs.txt analysis/SWBFRONT.ELF-stringrefs.txt
awk 'NR>=690000&&NR<=760000&&/0x3d[,;)]/' analysis/SWBFRNT2.ELF-decomp.c   # no FOGCOL
# ELF bytes
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 6ef3dc 2
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 64feb0 6   # RedModel vtable
# disc bytes, all maps
python3 engine/artifacts/lead_tmp/s12/re/fog_scratch/skydump.py $(cat engine/artifacts/lead_tmp/s12/re/fog_scratch/maps.txt)
python3 engine/artifacts/lead_tmp/s12/re/fog_scratch/fogtable.py
```

## 8. Not closed

- **VU1 side.** The exact clamp of `f`, whether the fogspread pass multiplies by vertex alpha, and the composite's
  texture function are in the microcode, not the ELF. The linear ramp and the colour are CPU facts; the blend is [D].
  **Partly measured 2026-09-28 (§9.3, [O]).** On BF1 KAS2's far water, with patched ramps:
  - the PS2's blend fraction is linear in f (a400/a800 = 1.84–2.05);
  - it caps at 0.93–0.98;
  - f uses planar view depth (the RAM matrix).
  Near models took no fog at all there (card FOGB). The microcode itself is still unread.
- **BF1 fog persists across maps.** No reset of `DAT_006ef3dc/e0` / `DAT_006ea980` exists in the decomp; a map with no
  `LowResTerrain` fog (RHN1, RHN2) inherits the previous map's fog on the disc (none after boot). [D — an indirect
  memset over 0x6ea980 was searched for by literal and not found; a Ghidra data-xref pass would settle it.] BF2's reset
  `FUN_00580d30` is reached only via `FUN_00175258` (17204/17262); whether that runs per level was not traced.
- **Far-layer assignment.** How a BF2 object ends up drawn with 0x20000 (per-object flags `obj+0x44` ORed with the
  flush mask in `FUN_005b0ab8` 746873; sphere cull against `FUN_00565140`) was not traced to the distance test.
- **Preimages** for 0x23e10337 (split-screen scope), 0x3c84cf36, 0xc15d0860, 0x6d9b436e remain unknown (~400
  candidates tried, `fog_scratch`).
  **Corrected 2026-09-23 (cleanse):** `TextureScale`
  (`re-lowr.md` §1; `mungefs::sky`).
- `DAT_0076c060` (BF2 `MaxDistance`) is read only by the low-res renderer (`+0x50` of `0x76c010`); the LOWR chunk
  format itself is untouched here.

# OPUS RE (loop lane 2, 2026-09-28, card KASF) - [O] UNAUDITED: no Fable pass
## 9. BF1 KAS2's `255,0,0`: authored, live on the PS2, out of reach — the port's red band is its CLEAR colour (card KASF)

The question (the maintainer's flythrough item 3; PXC FINDINGS F2/F23/F26): is §6's KAS2 row (fog ON, 255,0,0, 1000..2992) real,
or an unset default the table read as a colour? And why does the port draw a pure-red band where the PS2 shows none?
Scratch, tools and every raw output are in `engine/artifacts/lead_tmp/re/kas2-fog/`: `checks.txt` holds the §5 checks
verbatim, `CLAIMS.md` is the verifier's brief (C1–C15).

**Answer.** The row is right, byte for byte, and the PS2 holds it live. KAS2's world fog IS red, 1000..2992.19 m of view
depth. [V][O] It never shows at any captured pose, because nothing the PS2 draws there is deep enough for a visible
tint. The port's pure-red band is not fog at all. It is the frame's CLEAR colour, which the port sets to the fog colour
(`game::viewer::clear_color`, marked "PROVISIONAL … our choice"). It shows through a gap below the horizon where the port
draws nothing. [V][O] The PS2 never clears to the fog colour: its clear packet writes RGBAQ 0, and the PS2 frames are
dark there. [V][O] The fix is build card **CLRB**. REFL (the sky shells the PS2 draws mirrored below this horizon) covers
the rest of that band.

### 9.1 The bytes, and the census [V][O]
- **KAS2.LVL**, its one `sky_` chunk at file 0x18ef54:
  - `DomeInfo/LowResTerrain/FogColor` is a DATA record at 0x18f368: hash 0x023b1fce, argc 4, arguments at 0x18f375 =
    `00 00 7f 43 | 00 00 00 00 | 00 00 00 00 | 00 00 00 43` = (255.0, 0.0, 0.0, 128.0).
  - `FogNear` at 0x18f338 = `00 00 7a 44` = 1000.0.
  - `FogFar` at 0x18f350 = `00 00 fa 44` = 2000.0.
  - Offset formula: argument i of a DATA record whose tag starts at file offset T sits at T + 13 + 4i (the 8-byte chunk
    header, the u32 hash, the u8 argc).
  - KAS2_H.LVL has the same three records at 0x185778 / 0x185748 / 0x185760.
  - Tool: `skyraw.py`, which walks the UCFB tree as `mungefs::content_chunks` does. The verifier re-read the bytes with
    `od`.
- **Census of all 323 LVLs on both discs** (`census.py`):
  - 66 `sky_` chunks in 51 LVLs.
  - Exactly 3 FogColor records with (255,0,0): BF1 BES1 `SkyInfo` (file 0xe110c, dead: see 9.2), and BF1 KAS2 and KAS2_H
    `LowResTerrain`. None on BF2.
  - A raw byte search for (255.0f, 0.0f, 0.0f) finds 709 hits: 654 in `fx__`, 50 in `hud_`, 5 in `sky_`. The 5 are the
    three records above, plus NAB1/NAB1_H, where the triplet straddles the tail of a `Color(128,255,255,0)` record.
  - BF1's placeholder `SkyInfo` colour is (128,128,128), in 14 of its 20 records. BF1's 20 `LowResTerrain` FogColors are
    art colours shared only by twins and by maps with the same sky.
  - So 255,0,0 is not an unset-field convention. It is one map's explicit four-argument record, alpha included.

### 9.2 The ELF path [V][O]
Checks 1–5 are pasted in `checks.txt`, and the disassembly is `eedis2.py`.
- **BF1 builds the FogColor hash once.** The only `lui 0x023b … ori 0x1fce` in the BF1 text is at 0x2e3278/0x2e327c,
  inside `FUN_002e29e0` (B1:270546). That is the record loop of the `LowResTerrain` scope (0xddf0c29e, B1:270346).
  - The value is in no data word, and `re_check absent` finds 1 hit.
  - So BF1 never reads `SkyInfo.FogColor`, and BES1's red is dead data (as §1 said).
- **Colour (C4).** The branch at 0x2e335c reads arguments 0/1/2 (sp+568/572/576).
  - Each is clamped to [0, 255.0] (`lui at,0x437f` at 0x2e3364) and truncated to a byte.
  - The 4th byte is forced to 0xff, and the 4th argument (sp+580, only when argc ≥ 4) is kept as alpha.
  - The word goes `FUN_00479dc0` → `FUN_00499798` → `DAT_006ea970` (B1:541002, 560142).
  - The decomp's `a local` blue channel is argument slot 2 (sp+576), not a stale variable.
- **Stretch (C5).** At the scope end (B1:270621–270623; 0x2e3448–0x2e3480), when (alpha − 1) & 0xff < 254 (`sltiu 254`
  at 0x2e3450), FogFar is set to near + (far − near)·(255.0/alpha).
  - The divide is a float `div.s` (0x2e3474).
  - For KAS2: 1000 + 1000·255/128 = **2992.1875**.
- **Writers and readers (C8, C9).**
  - `DAT_006ea970` is stored only by `FUN_00499798` (swl/swr at 0x4997a8/ac; addrscan + widescan).
  - `FUN_00499798`'s only caller is the thunk `FUN_00479dc0`. The thunk's two callers are this loader (0x2e3418) and the
    `setfogcolor` console handler `FUN_002e27a8` (0x2e27f4, B1:269881). No BF1 Lua file mentions "fog".
  - The colour is read only via `FUN_004997b8`. Its 6 jal sites are fog-pass colour setups that halve the colour to GS
    scale: the sheet `FUN_00488268`, the water far passes `FUN_0048b280`/`FUN_0048b9c0`, `FUN_004b1a20` and the segment
    draw `FUN_004d0108`, plus `FUN_00479e20`, a getter with no caller.
  - There is no function pointer to any of them. **Nothing in BF1 clears a target to the fog colour.**
  - The rows 0x6ea980–0x6ea9bf are stored only by `FUN_00499678`/`FUN_00499708` and loaded only by `FUN_0049b9a8`'s four
    lqc2 (0x49ba78–0x49ba90, B1:561263–561266).
  - `FUN_0049b9a8` rebuilds the fog matrix at 0x6ea8b0 (B1:561301) under `if (DAT_00522ac8 != 0)` (the active camera), from
    the view loop `FUN_00479458` (540726). "Every frame" is shown by the patched states (9.3), not by the text.
- **The PS2's clear (C14).** `FUN_0049aa68` (B1:560843) builds TEST_1 0x30000, PRIM 6 and RGBAQ **0**
  (`sd zero,128` at 0x49ab8c, register 1 at 0x49ab94): a black, alpha-0 sprite fill.
  - It has 4 jal sites: B1:561508 in `FUN_0049bf50`, and B1:565305/565568/565948 in the frame function `FUN_004a57e0`.
  - BF2's `FUN_00581830` (B2:716207) is the same packet (`re-sky-pass.md` §5.2 names it as the far target's clear).
  - Which BF1 target each site clears is not traced. 565568 runs only when `DAT_00522ac0 == 0` (ELF 1, no writer), so it is
    skipped in these states.

### 9.3 The PS2 running the map [V][O]
- **Live RAM, all 8 BF1 KAS2 save states** (`fogram.py`):
  - `DAT_006ea970` = 0xff0000ff; near 1000.0; far **2992.1875**.
  - Rows 0x6ea9a0/9a4 = 1/1992.1875 and 0x6ea9b0/9b4 = −0.501961.
  - `DAT_00522b54` = 1 (ELF default 1; its only writer is `FUN_004a8080`).
  - Sheet MaxDistance 2000; FarSceneRange 1500.
- **The whole BF1 table (C7).** One in-level state per map: the live colour, near and far equal §6's BF1 rows on all 15
  maps that have a state (every map but TAT1). That includes the alpha-stretched fars BES2 972.5, GEO1 1096.88,
  KAM1 1356.25, KAS1 4750, NAB1 1297.66 and YAV2 1196.88. RHN1/RHN2 hold 0 (off).
- **Planar view depth.** The fog matrix in RAM (0x6ea8b0, the Beach Head state) is fwd/1992.19 per world axis with offset
  (−fwd·eye − 1000)/1992.19 = −0.6387342. So f = (planar view depth − near)/(far − near).
  - The port fogs by RADIAL distance (`shader.wgsl:143`), a small deviation (FOGN/FOGB).
- **Positive control (patched states, `ps2exp/`).** Copies of the Beach Head `v1_sea` state were made with the fog patched
  in EE RAM, booted and re-saved about 12 s later. The patch was still in RAM, and the camera was the same to 1.5e-8.
  - A **0..400 m ramp** turns the far water saturated red. With the colour patched to (0,255,0), it turns saturated green
    (+138.8 levels of green excess over the band).
  - The sky shells and the near ground are unchanged.
  - **The natural ramp with a green colour changes nothing** (−0.34 levels, noise).
  - Ramps 0..200 / 0..400 / 0..800 m, far-water rows 150–161: a400/a800 = 1.84–2.05, where linear predicts 2.00. The
    fraction caps at 0.93–0.98.
  - This is the first measurement of the VU1 side §8 leaves open, for the far water only.
- **Deep views.** At Dock Two's spawn the footprint reaches 1641 m of view depth inside the PS2 frustum.
  - A (0,0,255) fog word on the natural ramp moves the frame by blue excess mean 0.008, 20 px > 16. A second unpatched
    reload moves it by −0.004 and 28 px, so the difference is reload noise.
  - Beach Head's spawn (1316 m) shows green excess mean 0.025, max 26.
  - All 21 PS2 frames in `pcsx2/ref/bf1_kas2/` have 0 pixels of R ≥ 200 with G, B ≤ 60.
- **Under the port's red pixels the PS2 is dark (C13).** `cp0_cp2_beachhead_v1_sea` median (10,20,11), 87 % with mean
  channel ≤ 30; `cp5_cp10_dock_one_spawn` (13,21,13); `cp0_cp2_beachhead_v4` (7,16,9).
- **Near models took no fog (secondary, contradicts §0.3 / §2.2's "every opaque model segment in BOTH scenes").**
  - Under the 0..200 m ramp (f = 0.05–0.2 at their 10–40 m), the bamboo stalks change by exactly (0,0,0).
  - The soldier's box changes by −0.13 (noise).
  - Only the far-water band (rows 128–~178) moves.
  - Which gate skips them (opacity, a near-scene rule, the matrix) is not read → card **FOGB**.
- **An oddity.** At Dock Two, only a (0,255,0) fog word lights three striped cyan holograms on the dock posts, identically
  on two reloads. The unpatched state and a (0,0,255) word leave them dark.
  - A fog lerp cannot raise blue, so the fog colour is consumed there some other way [H]. With the shipped red they stay
    dark, as on the disc.

### 9.4 The port, measured (card question (a)) [V][O]
Tool: scratch example `kasf_probe.rs` on main dae70c5; analysis in `probe_measure.py`.
- **As shipped:** fog (1,0,0), near 1000, far 3165.42 = 1000 + 1992.19/0.92 (`look.rs:606`, `FOG_DENSITY_SCALE` 0.92 at
  `:373`, the maintainer's 2026-08-31 request). Clear (1,0,0,1) (`clear_color`, `look.rs:296–301`).
- **(a) The blend is as specified.** On terrain, t = (d − 1000.8)/2161.3 fits 2,664 pixels at 1050–1700 m (d = radial
  distance, calibrated with a 0..4000 m white-fog render).
  - At 1450–1550 m, t = **0.2346**. The port's ramp gives 0.2309 and the ELF's 0.2510, the gap being the deliberate 0.92.
- **The red band is the clear colour.** At the PXC poses `v1_sea` and `dock_one_spawn`, 4,514 and 1,082 pixels are exactly
  (255,0,0) (rows 128–135, 128–131). **Every one** becomes exactly (0,255,0) when only the clear colour passed to
  `render_frame` changes.
  - The port's water there is 137 cells of 128 m, x −1024..1024, z −128..1024 (`water.glb`'s indexed cells; the
    accessor bounds are ±2048).
  - The heightfield and sheet also end at the ±1024 footprint. Nothing covers the rows between that edge (725 m from Beach
    Head) and the dome's bottom ring (KAS2's dome band starts at 0.0°), so the fog-coloured clear shows.
  - A drawn fragment cannot reach (255,0,0): radial fog against a planar 2000 m clip gives t ≤ 0.60 at the frame corner.
- F2's grey-white rows (210,211,215) at Dock One are drawn pixels (unchanged by the clear or by fog off): a sky shell.

### 9.5 Still [H]
- Whether any KAS2 view shows a partial tint. It would need geometry past 1000 m of view depth that is not occluded: the
  sheet to MaxDistance 2000 (f ≤ 0.50), the far water to 0.9·1500 = 1350 m (≤ 0.18), models to 1500 (≤ 0.25).
- What BF1 clears each target to, beyond `FUN_0049aa68`'s four sites. No other clear path was searched.
- What fills the PS2's band exactly. It is dark (the black far target, and REFL's mirrored shells); its colour at each
  pose is the build's pairs to beat.
- The green-only holograms (9.3) and the near-model gate (FOGB).

### 9.6 Verifier
One foreground Opus verifier on C1–C15 (claims and citations only), with its own scripts in
`lead_tmp/re/kas2-fog/verify/`: **12 SUPPORTED, 3 PARTLY, 0 NOT SUPPORTED**. The three PARTLY corrections are folded in
above:
- **C9:** four lqc2 loads, not one; the rebuild is gated on the camera; "every frame" is empirical.
- **C12:** the port's bound is t ≤ 0.60, not 0.46 (radial fog, planar clip).
- **C15:** `water.glb`'s accessor bounds are ±2048; there are 21 PS2 frames, not 13; and the footprint does reach 1000+ m
  of view depth at 12 of 21 poses. Hence the deep-view test in 9.3.

It also found that `addrscan.py` reads BF1's gp as 0 from `.reginfo` (the real gp is 0x56c3f0, set at 0x2e8b08). No
target here lies within gp ± 32 KiB.
