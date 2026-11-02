# LEAD VERIFICATION (Fable lead, session 14, 2026-09-08): VERIFIED, with the session-14 verifier's five corrections of the session-13 draft (W1–W5 in lead_tmp/s14/re/REPORT_lowr_hoth.md) carried into this document.
Re-read by the lead: the extent census re-run (`lowr_census.py`; the lead's output is identical: extent/W = 1.000 on
all 16 sheets); SWBFRONT.ELF-decomp.c 536980–536984 (the WATR case), 538727–538735 (WMAP → DAT_005648dc),
550903–550916 (FUN_0048b1e0 is gated on it — the WATER low-res pass, not the heightfield), 549136–549140 (the far
branch draws FUN_00488268 only); SWBFRNT2.ELF-decomp.c 706497–706501 (the sheet, then return), 416064–416067
(MaxDistance default 0x44fa0000 = 2000). NOT re-read by the lead: the loader/format lines (§1), the fog-pass wiring
(549414–549415, 706769–706771), the camera far-plane claim (540930–540937 read, not traced to the camera), the BF2
DAG1/NAB2 no-scope → 0 chain. Decisions: LOWR COMPLEMENTS queue item 5 (the far cull); the razor-horizon defect
group is item 5's, not LOWR's; item 5's terrain clip moves to NearSceneRange[2] (PROVISIONAL).
Verification of `lead_tmp/s13/re/lowr.md` (Fable RE agent, session 13, stopped early): every cited line re-read, the
disc census the s13 report could not run was run (`lowr_census.py` → `lowr_census.txt`, `lowr_patches.txt`, this dir).
Corrections to the s13 text: (1) `FUN_0048b1e0`/`FUN_0048b9c0` is the WATER's low-res (far) pass, not "the real
terrain's far pass" — the real terrain is drawn only in the near scene, in BOTH games; (2) BF1 `+0x64` (grid) and
`+0x50` (MaxDistance) ARE zeroed at every tern load (545913 / 545885 via 546002), so there is no stale grid and a
scope-less map has MaxDistance 0; (3) BF2's MaxDistance default is 2000 (416065), and BF2 DAG1/NAB2 ship a `LOWR`
that is never drawn; (4) "cosmetic at most" is wrong — the sheet is the ONLY terrain either PS2 draws past the near
scene. Preimages closed: 0x6d9b436e = `TextureScale`, 0x3f2e0eaf = `DetailTexture`, 0x6021b96f = `DetailTextureScale`,
0xa01d9243 = `DisableLowRes` (water). Tags below are mine; [V] = read at the cited line or measured on the disc.

# RE: the low-res terrain sheet `LOWR` — what the PS2 draws past the near scene (HOLD 10(i))

Corpora: `analysis/SWBFRONT.ELF-decomp.c` (BF1, "B1:"), `analysis/SWBFRNT2.ELF-decomp.c` (BF2, "B2:"), ELF bytes via
`scripts/tools/elfrd.py`, disc bytes read by `lowr_census.py`, sky records from
`lead_tmp/s12/re/fog_scratch/skydump_all.txt`. Hashes = fnv1a over `byte|0x20` (`classdb.rs::fnv1a_lowercase`).
Tags: [V] read in the decomp/ELF/disc, [D] derived, [H] hypothesis.

## §0 Headline

1. **`LOWR` is a coarse triangle-strip mesh of the terrain's own footprint — a far LOD, not an outside fill.** [V]
   Census of all 32 shipped terrains (`lowr_census.txt`): 16 carry `tern → PCHS → LOWR` (BF1 BES2 GEO1 HOT1 KAS1 KAS2
   NAB1 NAB2 TAT1 YAV2 and the `_H`/`H` twins; BF2 DAG1 END1 GEO1 HOT1 KAS2 NAB2 POL1); on every one `INFO.extent`
   equals `gridUnit·gridSize` (ratio 1.000; 2048 m on the 256×8 maps, 1024 on BF2 KAS2/POL1, 512 on BF2 DAG1/NAB2).
   The sheet's vertices span x ∈ [−half, +half], z ∈ [−(half−gu), +half] (POSI u16 z min = 255 on gu 8 maps =
   −1016.0 m) against the terrain's x ∈ [−half, half−gu], z ∈ [−(half−gu), half]: the same footprint plus ONE vertex
   column at +x. Sheet vertex height minus the HGT8/HEXP height at the same XZ: median |dy| 0.00–1.08 m per map,
   max 4–43 m (the coarse patches cut corners on ridges). No map's sheet reaches beyond its terrain.
2. **Both games draw the sheet in the FAR scene only, and it is the only terrain there.** BF1: the terrain driver's
   far branch (`(flags & 0x20000)`, B1:549138/549319) calls the sheet renderer and nothing else; the real terrain is
   the near branch (549138–549315) drawn with the camera frustum whose far plane is `NearSceneRange[2]` (540932–540937).
   The other 0x20000 call in the sky tick, B1:280299 `FUN_00475940` → `FUN_0048b1e0` (550903), is the WATER's low-res
   pass (§2.3) — the s13 report's "real terrain's far pass" was a misread. BF2: the driver `FUN_00571bd0` draws the
   sheet and returns (B2:706497–706501) unless a debug toggle (`DAT_0060c1c8`, 707249) or the sky object's byte
   `+0x2854` (`DAT_0060bedd`, only ever written 0 at 416042) is set. [V]
3. **Range: from `NearSceneRange[1]·S` to `LowResTerrain.MaxDistance`; MaxDistance defaults to 2000 in BOTH games**
   (B1:269943, B2:416065), authored BES2 400, KAS1 1000, NAB2 2000, TAT1 3000 (BF1), POL1 1500 (BF2). Z-tested and
   Z-written (TEST 0x5010b), alpha-tested at 16/255, base colour unity, ALWAYS with the `fogspread` pass in BF1
   (549414–549415); in BF2 with `fogspread` iff the BF2 fog flag (`FUN_005805b8`, 706769–706771). [V]
4. **Where it is absent, nothing replaces it.** No `LOWR`: BF1 BES1 END1 KAM1 RHN1 RHN2 TAT2 YAV1; BF2 FEL1 TAT2 YAV1
   (and the model-only stubs). BF2 DAG1/NAB2 ship a sheet but author no `LowResTerrain` scope: the tern load zeroes
   MaxDistance (B2:703198 via 703213/703297) and only the scope end restores it (416256–416258), `tern` precedes `sky_`
   in every LVL — so their sheet fails the gate `0 < MaxDistance` (706768) and is never drawn [V loader, D order].
   END1/YAV1 author `Texture('end1_lowresterrain'/'yavin1')` and the `tex_` exists in both discs' LVLs — but no `LOWR`.
5. **Consequence for the port (PROVISIONAL, for the lead):** the "flat plane to a razor horizon" on END1/RHN2/DAG1
   and the MUS1/UTA1 band are NOT a missing sheet (no sheet is drawn on those maps on the PS2 either); the sheet
   changes BES2, NAB2, KAS2 and the eleven other sheet maps, INSIDE the footprint, past the near scene. It complements
   the far cull (queue 5) — and says the real terrain's own clip is `NearSceneRange[2]`, not `FarSceneRange` (§5).

## §1 The chunk (loader-verified; disc-verified by the census)

Tree `tern → PCHS → { PTCH × n², LOWR → { INFO, PCHS → LPTC × grid² } }` (B1:546096 `0x52574f4c`, 546141
`0x53484350`, 546146 `0x4354504c`; B2:703386/703436/703440). `mungefs/tests/suite/tat1_lvl.rs:426–478` already pins the
tree and TAT1's INFO. [V]

**`LOWR INFO`** (B1:546101–546127; B2:703394–703408): `u32 grid` → obj+0x64 (546104), `u32 res` → +0x60 (546106),
`f32 extent` → +0x5c (546108), `+0x68 = grid·res/extent` (546109–546110), `f32 ymin, f32 ymax` (546111–546112).
Sheet box (546113–546116 → +0x30 min / +0x40 max, stored 546118–546127): *(decompiler detail omitted)*,
*(decompiler detail omitted)* with `half = gu·gridSize/2` (B1:536817 `DAT_00564698`),
`half−gu` (536821 `DAT_0056469c`), *(decompiler detail omitted)* (546000) → **x, z ∈ [−half, extent−half]**. Patch array
`grid²×0x30` (546129). Disc: grid 8–18, res 5–11 (= `PatchResolution`), extent = W on 16/16. [V]

**`LPTC`** (B1:546157–546240; B2:703449–703534): `INFO` 6 f32 `(x0, x1, y0, y1, z0, z1)` stored as box
`(x0+c, y0, z0+c)..(x1+c, y1, z1+c)`, `c = extent/2 − half` (546148–546154; = 0 on every shipped map) — culling only
(549558). `POSI` = size/6 × `u16 x, y, z` (546183–546191). `STRP` = size/2 × u16 (546193–546198): bit 15 = strip
flag → VU word 0x8000, and 0x4000 on every second vertex (546214–546233); `index & 0x7fff` selects the triple
(546219). **Both of those 0x4000s are in the word the LOADER BUILDS for the VU, not in the disc's bytes**
(measured 2026-09-17 by queue L's build and reproduced at its landing, `engine/artifacts/lead_tmp/lowr-sheet/strp_bits.py`):
over **all 21 shipped sheets on both discs — 2,893 `LPTC` patches, 415,277 `STRP` u16s — bit 14 is set in
exactly 0**, and `index & 0x7fff` overruns its patch's `POSI` count in exactly 0. Bit 15 is set in 67,202 of
them (the strip restarts). So a port that masks with `& 0x7fff` and unstitches on bit 15 is exact on shipped
data, and any reader that expects 0x4000 to mark alternate vertices will find none. [V] **UV generated**, no colour, no normal: `uv = ((x·k) & 0xffff) | ((z·k) << 16)`, `k = (extent/65536) · S58 ·
2048 · (1/gu) / gridSize` (546211–546212; B2:703506–703507), S58 = obj+0x58 = `DAT_0069c2c8` = key 0x6d9b436e
= **`TextureScale`** (preimage by hash), default 1.0 (269942; reset 545861/545886). Packet
`…
per map 2 326–21 337 vertices, 4 458–42 970 indices; POSI u16 x,y,z min (0, 0, 255|511) max (65534|65535, ·, 65535).
Dequantisation: the renderer sets the VU box to the SHEET box (549503; B2:706855). The VU maps u16/65536 over the
box [D]: the PTCH path's box is padded by 1.0000153 = 65536/65535 (546004 `DAT_00522a18`), which only makes sense
for a /65536 dequant; the census's dy statistics were computed that way and land on the terrain.
`TextureScale`: BES1/BES2 20, END1/YAV1 19 (both discs), else 1 → S58 repeats across the sheet iff 2048 UV units =
one repeat [H — by analogy with the PTCH packer's 0x800 per cell at 548696; the VU program was not read].

## §2 BF1 consumer

2.1 **Call chain.** Sky tick `FUN_002f2308`: B1:280296–280298 `if (DAT_005229a8) FUN_00481300(0x20000)` → `FUN_00487ad8
(0x69c270, flags)` (545762) → 549131 terrain-enabled gate, 549138 `(flags & 0x20000) == 0` = near-terrain patches,
else 549319 `FUN_00488268(obj)` = the sheet. Object `0x69c270`: +0x50 MaxDistance (`DAT_0069c2c0`), +0x54 texture hash
(`DAT_0069c2c4`), +0x58 TextureScale (`DAT_0069c2c8`), +0x5c..+0x68 `LOWR INFO`, +0x2c patches, +0x30/+0x40 box. [V]
2.2 **Renderer `FUN_00488268` (549331).** Gate 549413 `0 < MaxDistance || grid < 1` (with grid 0 the loop is empty →
drawn iff MaxDistance > 0). Program 549414–549415 `FUN_0049eb90(texture, "fogspread")` — 0x558d98 = "fogspread"
(stringrefs 6455) — unconditional. GS: pass-1 TEST 0x5010b (549463: ATE GEQUAL AREF 0x10, AFAIL KEEP, ZTE, ZTST
GEQUAL), fog pass ALPHA 0x144 (549474) / TEST 0x50000 (549483); debug variants under `DAT_00522a48..54`.
**Corrected 2026-09-29 (card REDB, `analysis/re-fog-amount.md` §2, verifier-found, [O]):** the "debug variants" are the
LIVE ones. `DAT_00522a4c/50/54` hold 1 in the ELF and in RAM with no writer (`DAT_00522a48` = 0), so the PS2 draws the
sheet with pass 1 {0x800800, TEST 0x5c10b} and fog pass {0x800944, TEST 0x5c000}. Both passes carry DATE = DATM = 1
(drawn only where the far target's alpha bit is set) and the VU cull flags 0x800800. The fog pass's second texture
`"fogspread"` is a texture (`re-fog-amount.md`). The per-map far scene (13 sheet maps) is card FARS; the sheetless maps
are FARC. Fog matrix
549496–549497 (`FUN_004a84a0`), fog colour halved to GS scale 549499–549502, base colour 0x80808080, VU box 549503.
Frustum 549507–549509: `FUN_00491148(DAT_00677660 · cam+0x90, MaxDistance, frustum)` — `DAT_00677660` is
`NearSceneRange[1]` (key 0x50551c9a at 271147 → `FUN_00479c10` 540918, stores b at 540930, c at 540931, and sets every
camera's far plane to c at 540932–540937; getter 540975), cam+0x90 = zoom S (re-sky-pass §1). Patch loop
549511–549575 in camera-sign order, box cull `FUN_004915a0` 549558, clip flag 0x100 for partial patches 549561,
submit 549569/549572. [V]
2.3 **What else is in the far scene: the WATER, not the terrain.** B1:280299 `FUN_00475940(0x20000)` = `FUN_0048b1e0()`
(538431). That function is gated on `DAT_005648dc` (550908), which is set at 538734 inside `FUN_00475e08` (538702) —
the parser the tern loader calls for tag `0x52544157` = `WATR` (536982–536983); it reads `WMAP` (0x50414d57, 538729)
and, per tile with bit 3 set, an `LPTC` through `FUN_00488f20` (538764–538767; 549961 — the s13 §6.4 "second LPTC
consumer": INFO 6 f32 / POSI / STRP plus a 17×17 LCG table = the water tile's wave phases [D]). Its config
`FUN_004763d0` (538852, called 369276) takes the water keys `Tile` 0x713a7cc9, `Velocity` 0x32741c32,
`PatchDivisions` 0x9036eb, `WindSpeed`, `WindDirection`, `MainTexture`, `FoamTexture`, `LODDecimation`, `OceanEnable`
(538912–538988, all by hash) and **`DisableLowRes` = 0xa01d9243 → `… default 1 at
538254; ELF 0x5229c4 = 01). `FUN_0048b9c0` (551182) scrolls its UVs (551367–551368) and, for the far call, takes the
frustum `(NearSceneRange[1], 0.9·FarSceneRange)` (551290–551294; `DAT_00522a7c` = 0.9 in the ELF, `DAT_00677668` =
`FarSceneRange` set from key 0x1192b7b 271104 → 540946). So BF1's far scene = the sheet (to MaxDistance) + the water's
low-res pass (to 0.9·FarSceneRange) + far-layer models; the heightfield ends at `NearSceneRange[2]`. [V]
2.4 **Resets.** Every tern load runs `FUN_00481590` (546002; 545898–545914): +0x50 = 0, +0x58 = 1.0, +0x54 = 0
(via `FUN_00481538` 545885/545886/545891) and +0x64 = +0x5c = +0x60 = +0x68 = 0 (545910–545913). The sky object
(`FUN_002e2888` 269917, built once at 269895) keeps +0xf4 (MaxDistance) across maps: a map whose scope omits
`MaxDistance` inherits the previous map's authored value if any [D — same family as re-fog §8; not measured]. [V]
**Corrected 2026-09-23 (cleanse): contradicted by the ELF.** 269885-269895 is `FUN_002e2810`, which allocates a
FRESH 0x108 object (`… the `sky_` key dispatcher
`FUN_002e44d8` calls it at ~271240 whenever key 0xaf8bb8ce (`Enable`) has bit 0 set (as `re-fog.md` ~:65-66
says). So a map that authors `Enable(1)` gets a new sky object and inherits nothing. [V — lines read by the
lead; `[O]`].
2.5 **Keys** (`FUN_002e29e0` 269983, per `sky_` 271581): `PatchResolution` 0x6ef738fb → +0xf8 (270537–270538),
`Texture` 0x3c6468f4 → +0xf0 (270544, 270576–270578), `TextureScale` 0x6d9b436e → +0xfc (270586–270587), `FogColor`
0x23b1fce (270546), `FogNear` 0xcc60f1a1 (270597), `FogFar` 0x7e28d534 (270604), `MaxDistance` 0xe6915cf6 → +0xf4
(270612–270613); scope end 270629–270630 → `FUN_00488b90` (549845–549849: `DAT_0069c2c8`, `DAT_0069c2c4`) and
`FUN_00488ba8` (549855–549859: `DAT_0069c2c0`). [V]

## §3 BF2 consumer — a twin, with one difference

Loader `FUN_0056bbf0` (B2:703226): reset `FUN_0056bb88` 703297 (703210–703221 → `FUN_0056bb08` 703198 `+0x50 = 0`,
703199 `+0x58 = 1.0`; 703219 `+0x64 = 0`); `LOWR` 703386; INFO 703396/703398/703400/703401–703408; alloc 703423;
`LPTC` 703440; POSI 703476–703481; STRP 703483–703486; UV 703506–703507 with `DAT_00661494` = 1/gu (694959) and
`DAT_006613fc` = gridSize (694904); packet 703534; *(decompiler detail omitted)* (703295). [V]
Driver `FUN_00571bd0` (706418; sky tick 426024 `FUN_0056b9a0(0x20000)` → 703098): under `(flags & 0x20000)` (706497),
*(decompiler detail omitted)* (706498–706501) — `DAT_0060c1c8` is a
debug variable (registered 707249), `FUN_0055db48` returns `DAT_0060bedd` = sky +0x2854 (694474–694488; written only
0 at 416042). Otherwise (debug) the heightfield is drawn in the far scene to +0x50 (706502–706508). **So BF2's far
terrain is the sheet alone** [V]. Renderer `FUN_00572458` (706715): gate 706768 (same), `FUN_005805b8` (706769–706771)
selects plain mode (706772–706795) or `fogspread` 0x64fc88 (706797–706852; stringrefs 5765), VU box 706855, frustum
706859–706863 `FUN_0057b990(DAT_0074601c · cam+0x140, MaxDistance)` (`FUN_00565120` 698787), loop 706864–706935. [V]
Sky object `FUN_003bcb08` (416024, built once 416002): +0x2850 TextureScale 1.0, **+0x2848 MaxDistance 2000.0**,
+0x284c PatchResolution 5, +0x2844 Texture 0 (416064–416067); `LowResTerrain` = 0xddf0c29e (416248) keys
416260–416281; scope end 416256–416258 → `FUN_00572fe0` (707200–707205: `DAT_0076c068`, `DAT_0076c064`),
`FUN_00572ff8` (707210–707214: `DAT_0076c060`), `FUN_0055db38(+0x2854)`. POL1's extras: 0x3f2e0eaf =
`DetailTexture` ('pol1_far_detail'), 0x6021b96f = `DetailTextureScale` (0.0625) — consumer not traced [V hash, H use].

## §4 Census (from `lowr_census.txt`; W = gu·gridSize)

| map | W | grid×res | extent/W | LPTC | verts | idx | sheet y / terrain y | dy med / max | MaxDistance |
|---|---|---|---|---|---|---|---|---|---|
| BF1 BES2 | 2048 | 10×10 | 1.000 | 100 | 12118 | 25040 | −58..−42 / −57..−38 | 0.10 / 6.7 | 400 |
| BF1 GEO1 | 2048 | 8×6 | 1.000 | 64 | 3136 | 5824 | −76..−8 / −72..−6 | 0.91 / 26.1 | 2000 (default) |
| BF1 HOT1 | 2048 | 16×8 | 1.000 | 256 | 21201 | 42778 | −20..113 / −19..114 | 0.82 / 43.2 | 2000 (default) |
| BF1 KAS1 | 2048 | 11×11 | 1.000 | 121 | 17424 | 36058 | −53..−26 / −53..−24 | 0.10 / 8.4 | 1000 |
| BF1 KAS2 (+_H) | 2048 | 10×8 | 1.000 | 100 | 8100 | 16400 | −11..22 / −12..22 | 0.10 / 9.8 | 2000 (default) |
| BF1 NAB1 (+_H) | 2048 | 18×5 | 1.000 | 324 | 11664 | 22356 | −141..−80 / −140..−80 | 0.26 / 5.2 | 2000 (default) |
| BF1 NAB2 (+_H) | 2048 | 10×10 | 1.000 | 100 | 12118 | 25022 | −63..−2 / −61..0 | 0.10 / 23.0 | 2000 |
| BF1 TAT1 (+H) | 2048 | 8×5 | 1.000 | 64 | 2326 | 4458 | −5..55 / −1..64 | 1.08 / 14.0 | 3000 |
| BF1 YAV2 (+_H) | 2048 | 10×6 | 1.000 | 100 | 4926 | 9162 | −63..−53 / −63..−50 | 0.10 / 7.0 | 2000 (default) |
| BF2 DAG1 | 512 | 8×5 | 1.000 | 64 | 2362 | 4522 | −3..4 / 0..6 | 0.10 / 4.0 | **0 — no scope, never drawn** |
| BF2 END1 | 2048 | 10×6 | 1.000 | 100 | 4910 | 9124 | −8..44 / −6..47 | 0.00 / 7.9 | 2000 (default) |
| BF2 GEO1 | 2048 | 14×8 | 1.000 | 196 | 16080 | 32525 | −109..30 / −100..30 | 0.57 / 41.9 | 2000 (default) |
| BF2 HOT1 | 2048 | 16×8 | 1.000 | 256 | 21337 | 42970 | −20..102 / −19..102 | 0.78 / 43.2 | 2000 (default) |
| BF2 KAS2 | 1024 | 14×8 | 1.000 | 196 | 15876 | 32144 | −1..143 / 0..145 | 0.05 / 26.3 | 2000 (default) |
| BF2 NAB2 | 512 | 10×10 | 1.000 | 100 | 12042 | 24865 | 0..8 / 2..8 | 0.00 / 7.3 | **0 — no scope, never drawn** |
| BF2 POL1 | 1024 | 8×5 | 1.000 | 64 | 2431 | 4631 | −11..84 / 0..87 | 0.42 / 25.3 | 1500 |

No sheet: BF1 BES1 END1 KAM1 RHN1 RHN2 TAT2 YAV1 (TEST); BF2 FEL1 TAT2 YAV1; BF2 COR1 DEA1 KAM1 MUS1 MYG1 TAN1 UTA1
have no heightfield. `tern` precedes `sky_` in all 32 LVLs. The authored textures exist as `tex_` on every sheet
map and on END1/YAV1 (both discs); BF2 DAG1/NAB2 author none.

## §5 Port rule (PROVISIONAL until the lead views goldens; the mechanics are [V])

```
bake:   parse tern/PCHS/LOWR: INFO{grid,res,extent,ymin,ymax}; per LPTC: INFO 6f (box, +c, c = extent/2 − half),
        POSI n×3u16 dequantised over the SHEET box [−half, extent−half] × [ymin,ymax] × [−half, extent−half]
        (u16/65536), STRP u16 strips (bit 15 = flag; mungefs::model::unstitch_strips convention);
        uv = (x_m − (−half)) / W · TextureScale, same for z  [H: 2048 units = one repeat]; texture =
        LowResTerrain.Texture (already in ManifestSky.low_res_terrain); unlit, colour 1.0; no normals.
draw:   far scene only — from NearSceneRange[1]·S to MaxDistance (default 2000 both games; 0 when no scope);
        depth test + write, alpha-test 16/255, the map's fog applied (never as sky). Absent LOWR → nothing extra.
        The heightfield itself is drawn only to NearSceneRange[2] on the PS2 (both games) — the near scene.
```
**Corrected 2026-09-23 (cleanse):** queue L built something else — the sheet's near edge is the port's own
(`game::viewer::lowres_range`) and it draws OPAQUE (`bake::lowres_glb`); both are open in queue CLN5.
**Corrected 2026-09-25 (queue CLN5, loop lane 1):** (a) **BF2 END1 ships and names a sheet but draws none**: it
authors `FarSceneRange(0)`, which clears the far-buffer composite (`re-fog.md` §2.5 [V]; this section's "maps WITHOUT
a drawn sheet" below already lists END1), so §4's END1 row is a sheet the disc HOLDS, not one it shows. The port
returns no sheet for a `FarSceneRange(0)` map [D]. (b) **Two sheets carry real alpha on the disc** — the earlier
"every sheet texture is alpha 255" read the BAKED KTX2s, which the bake had forced opaque: disc `tex_` mip 0, texels
below 16 — BF1 KAS1 `kashyyyk1` 62,571 / 65,536, BF1 KAS2 `kas2` 27,482 / 65,536; the other twelve 0
(`engine/artifacts/lead_tmp/cln5-lowr/sheet_alpha_disc.txt`, `mungefs::texture::Texture::rgba8` over every `tex_`)
[V]. So the AREF 0x10 test above cuts real holes there; the port keeps that alpha and tests it (`render`'s
`fs_far_sheet`, disc A ≥ 32 — the AREF composed with the CLUT upload, PROVISIONAL [D]). The draw is UNLIT, as
this section says. What the PS2 shows through those holes (the water's low-res pass, §2.3) the port does not draw
yet: behind `OPENBF_FAR_CULL`, the holes show the clear colour.

Effect: inside the footprint, past the near scene, the PS2 shows this flat-textured, fogged sheet instead of the
heightfield. It does not touch the terrain EDGE (same footprint) — the razor horizons on maps WITHOUT a drawn sheet
(END1 ×2, RHN2, DAG1, MUS1/UTA1's shell band, KAM1) remain the far cull + fade (queue 5/6). The far cull's terrain
clip distance, per §2.2/§2.3, is `NearSceneRange[2]`, not `FarSceneRange` — the lead's call for queue item 5.

## §6 Preimages (all by fnv1a brute force over ≤ 3 tokens, `lowr_census.py`'s neighbour one-liner in the report)
`0x6d9b436e TextureScale`, `0x3f2e0eaf DetailTexture`, `0x6021b96f DetailTextureScale`, `0xddf0c29e LowResTerrain`,
`0x50551c9a NearSceneRange`, `0x1192b7b FarSceneRange`, `0xa01d9243 DisableLowRes` (+ the water keys in §2.3).

## §7 Reproduction
```
cd <repo>
sed -n 546096,546127p analysis/SWBFRONT.ELF-decomp.c; sed -n 546141,546240p analysis/SWBFRONT.ELF-decomp.c  # loader
sed -n 549413,549415p analysis/SWBFRONT.ELF-decomp.c; sed -n 549496,549509p analysis/SWBFRONT.ELF-decomp.c  # renderer
sed -n 549131,549138p analysis/SWBFRONT.ELF-decomp.c; sed -n 549318,549320p analysis/SWBFRONT.ELF-decomp.c  # driver
sed -n 280296,280299p analysis/SWBFRONT.ELF-decomp.c; sed -n 538428,538433p analysis/SWBFRONT.ELF-decomp.c
sed -n 550903,550916p analysis/SWBFRONT.ELF-decomp.c; sed -n 536980,536984p analysis/SWBFRONT.ELF-decomp.c  # WATR
sed -n 538727,538735p analysis/SWBFRONT.ELF-decomp.c; sed -n 538958,538962p analysis/SWBFRONT.ELF-decomp.c  # water
sed -n 545898,545914p analysis/SWBFRONT.ELF-decomp.c; sed -n 545879,545894p analysis/SWBFRONT.ELF-decomp.c  # resets
sed -n 269941,269944p analysis/SWBFRONT.ELF-decomp.c; sed -n 271147,271151p analysis/SWBFRONT.ELF-decomp.c
sed -n 540918,540937p analysis/SWBFRONT.ELF-decomp.c
sed -n 706497,706508p analysis/SWBFRNT2.ELF-decomp.c; sed -n 694474,694488p analysis/SWBFRNT2.ELF-decomp.c  # BF2 gate
sed -n 416042,416042p analysis/SWBFRNT2.ELF-decomp.c; sed -n 416064,416067p analysis/SWBFRNT2.ELF-decomp.c  # BF2 defaults
sed -n 703198,703199p analysis/SWBFRNT2.ELF-decomp.c; sed -n 703210,703221p analysis/SWBFRNT2.ELF-decomp.c
grep -n '558d98\|64fc88' analysis/SWBFRONT.ELF-stringrefs.txt analysis/SWBFRNT2.ELF-stringrefs.txt        # fogspread
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 5229c4 1                 # = 1
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 522a7c 1                 # = 0.9
python3 engine/artifacts/lead_tmp/s14/re/lowr/lowr_census.py                                                # the census
```

## §8 Not closed
- The UV unit (2048 = one repeat?) — VU1 program not read [H]; the port can pin it against `tat1`'s texel size.
- Cross-map persistence of the sky object's MaxDistance/TextureScale (+0xf4/+0xfc; BF2 +0x2848/+0x2850) when a
  later map's scope omits the key — [D] from the ctor being built once; a PCSX2 check or a reset site would close it.
  **Corrected 2026-09-23 (cleanse): contradicted by the ELF** — see the matching erratum in §2.4.
- `DetailTexture`/`DetailTextureScale` consumer (BF2 POL1 only).
- The water's low-res pass (`FUN_0048b9c0`, BF2 twin `FUN_005755b0` per re-sky-pass) is named here, not decoded;
  `analysis/re-fog.md` §2.2's table row calls `FUN_0048b9c0` "terrain" — it is the water.
