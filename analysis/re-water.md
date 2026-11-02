# LEAD VERIFICATION (Fable lead, 2026-09-09, session 17)
**Lead-REPRODUCED:** the disc `fx__ Effect("Water")` census on KAM1, KAS2 and YAV1 (`engine/artifacts/lead_tmp/s17/re/fxwater.py`, read-only) — KAM1 `MainTexture('kam1_water_2')`, `FoamTexture('kam1_water')`, `OceanEnable(1)`, `Tile(4,4)`; KAS2 `OceanEnable(0)`, `Tile(2,2)`, `MainTexture('kas2_water')`; YAV1, rerun by the lead on the map LVL `extracted/bf1/lvl/DATA/_LVL_PS2/YAV/YAV1.LVL`, `Tile(2,2)`, `OceanEnable(0)`, `DisableLowRes()`, `Velocity(0.1,0.02)`, `MainTexture('yav_water')`.
**SCOPE OF THIS VERDICT (added 2026-09-17 by the cleanse):** it covers the 2026-09-09 body only. §4's CORRECTION and the
patch-cell geometry added 2026-09-17 by queue WP are `[O] UNAUDITED` (no Fable pass — RE_LOG 2026-09-17), and §8's
amplitude estimate is superseded by OC's lead measurement (see §8).
**Corrected 2026-09-23 (cleanse):** the other way round — §3a (queue OCA) found §8's metre-scale estimate RIGHT
and §3's `1/(N·M)` wrong; queue OCB landed the unnormalised transform (`efab925`).
**Lead RE-READ (the load-bearing hops):** the port's `mungefs::water` `height: le::f32_at(rest, 4)` (cite the symbol — `7ef36a8` moved it from `water.rs:201` to `:241`) (the f32 after the u32 — the port reads the right field); the BF1 `CPAT` tile loop 551346–551359 (a tile with `+0x24 == 0` takes the flat template `FUN_0039cdc8` / *(decompiler detail omitted)*, else *(decompiler detail omitted)* on the patch packet — a patch REPLACES, it does not add); the far distance `DAT_00522a7c * FarSceneRange` at 551290–551293 (the 0.9 value of that constant is the agent's ELF read).
**NOT re-read — the AGENT's claims alone:** everything VU-side — the 16-entry per-vertex diffuse LUT and its alpha, the additive speckle pass, KAM1's Tessendorf FFT ocean, and the GS words quoted with them. They are [V] over the decomp, not over a second reading.
**VERDICT:** lead-verified on the hops above. HOLD 10(f) and HOLD 19(c) are ANSWERED (§3/§4 and §1/§2); HOLD 4 is AMENDED — the `fx__` chunk's `Effect("Water")` records are released to Opus (§5's 27 keys), every other `fx__` effect stays held; the water VU1 microcode and KAM1's ocean become HOLD FOR FABLE 21. Port work: queue items W1, W2, W3, WL. Everything below this header is the RE agent's report, verbatim.

# RE: WATER — fog, level, surface, CPAT (Fable RE agent, session 17, 2026-09-09) — lead-verified on the cited hops (see header)
Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1:), `analysis/SWBFRNT2.ELF-decomp.c` (B2:); ELF bytes via `elfrd.py` (BF1 and BF2 both map
`off = va − 0x160000 + 0x1000`, one PT_LOAD each); disc bytes by `lead_tmp/s17/re/` scratch walkers (`watr.py`, `fxwater.py` — the
`fx__` walker reuses skydump.py's DATA/SCOP reader). Hashes fnv1a over `c|0x20`. Tags: [V] read at the line / measured, [D] derived, [H] guess.

## Load-bearing citations (the lead should re-read these eight)
1. B1 `FUN_00475e08` 538702–538850 — the `WATR` parser: `LAYR` keeps ONLY the height (538743) and DROPS the u32, the two f32 pairs and
   the four colour bytes (538742, 538744–538751); `CPAT` is walked per `WMAP` cell with **bit 3** set, high nibble = even x (538764–538767);
   `INFO` = grid, cells/byte(2), cell, halfX, halfZ, INFO5 (538791–538796); finalise: Tile×INFO5, Velocity/INFO5, PatchDivisions×INFO5 (538831–538838).
2. B1 `FUN_004763d0` 538852–539000 + `FUN_00489c40` 550335–550410 — the config keys; they come from the LVL's **`fx__` chunk, `Effect("Water")`**
   (disc: KAM1.LVL `fx__` @0x57960, KAS2 @0xad830, YAV1 @0x6b06c), NOT from `sky_` and NOT from `LAYR`. All 27 keys named (§5).
3. B1 `FUN_0048b1e0` 550903–550919 — the driver: near pass always; far pass iff `DAT_005229c4` (=1 unless `DisableLowRes`, 538960–538961);
   `OceanEnable` (`DAT_005647b8`) → `FUN_0048b280` instead. Near call = `FUN_00478d50` 540292 (WaterRenderable, registered 538828 with sort key 0x186a1); far call = sky tick 280299.
4. B1 `FUN_0048b9c0` 551186–551520 — the flat-water renderer for BOTH passes (param 1 = near, 0 = far): far frustum `[NearSceneRange.b, 0.9·FarSceneRange]`
   551290–551294; main layer 551303/551306 (no fog program), record select 551330–551339, tile loop 551343–551359; far speckle + `fogspread` 551405–551448;
   near speckle cross-fade 551452–551479 (no fog); VU microcode 0x515410 in (551546) / 0x514ec0 out (551518).
5. B1 `FUN_0048fca0` 553341–553369 — the GS records: 0x69c390 near main = {0x4144, TEST 0x50000, 0x58}; 0x69c3a0 far main = {0x4144, 0x51001, 0x58};
   0x69c3b0 speckle = {0x4148, 0x51001, 0x58}; ocean 0x69c3c0/3d0 = {0x4100, 0x50000, 0x18}, 0x69c3e0 foam = {0x100144, 0x50000, 0x58}. Record layout per
   `FUN_0039c1a8` 388492–388540: +0 ALPHA|VUflags, +8 TEST, +0xc prim flags (0x40 = ABE, re-material-flags line 112).
6. B1 `FUN_004770a8` 539201–539341 — the per-frame cull that builds the ≤1000 tile records (0x30 B at `DAT_005648d8`): tile y = `height + bob + 0`
   (539266, 539322), patch pointer → tile+0x24 (539306–539324), box = 2 cells centred on the cell (539327–539330); wet test `FUN_00477458` 539345–539356 = nibble ≠ 0.
7. B1 `FUN_0048c4f0` 551525–551620 + `FUN_0048d068` 551904–551970 + `FUN_0048cec8` 551854–551903 — the VU constant block 0x754a30 (23 qwords, uploaded
   551622 opcode 0x785): light vector, `LightAzimAndElev`, `SpeckleCoordShift`, SpecularColor, SpeckleAmbient/SpeckleSpecular, then a 16-entry
   Border→Min→Max diffuse LUT (split `DAT_00522a74` = 5, ELF) and a 16-entry Min.a→Max.a alpha table. `DAT_00522a6c/70` = 1 in the ELF → shipped path.
8. B2 twins: parser `FUN_00560f40` 696348–696364 (LAYR: height only), 696369–696399 (CPAT on bit 3), 696405–696418 (INFO); config 696472–696600
   (+ `OscillationEnable` 0x12e39af3, `SpecularEnable` 0xd4ddec5f); driver `FUN_005755b0` 708251–708272; renderer `FUN_00575fd8`: far speckle + `fogspread`
   708831–708834 **gated on `SpecularEnable`** (near speckle gated too, 708897); records `FUN_0057a3a8` 710717–710745 (identical words). ELF: 0x60bee9 = 1 (low-res on), 0x60beea = 1, 0x60c1f8 = 0.
   **Corrected 2026-09-25 (`re-water-flags.md` §8, `[O]`):** the LAYR/CPAT/INFO lines 696348–696418 are in **`FUN_005609f0`**
   (header 696265), the WATR chunk parser; `FUN_00560f40` (header 696420) is the `fx__` config parser and ends at 696590.
   Both BF2 flags default to 1 at the level reset `FUN_005601a8` (695896, 695905).

## 1. Fog and distance
- **Near water (both games): NO fog.** The near branch of `FUN_0048b9c0` never loads `fogspread` (0x558e20 appears only at 551406, inside *(decompiler detail omitted)*);
  B2 `FUN_00575fd8` likewise only at 708833 inside *(decompiler detail omitted)*. The near main layer = one program (mode 8, `FUN_0049e2e0(MainTexture)` 551306),
  ALPHA 0x44 (Cs·As + Cd·(1−As)), TEST 0x50000 (Z-tested, **Z-written**), + the speckle layer ALPHA 0x48 additive, TEST 0x51001 (no Z write). [V]
  So re-fog §2.2's BF1 row "far pass only" is right for the water, and the near water is unfogged like BF2's whole near scene.
- **Far water = the same tiles again, past `NearSceneRange.b`, to `0.9·FarSceneRange`** (551290–551294, `DAT_00522a7c` = 0.9 ELF; ocean 0.8, 551039),
  drawn ONLY when the map does not author `DisableLowRes` (550908). Three sub-passes per tile: main (mode 10, `FUN_0049e240` 551303, ALPHA 0x44, TEST 0x51001 =
  Z-tested no Z write, NOT fogged), speckle frame `SpeckleTextures[(t·fps) mod n]` (551405–551407, additive 0x48), then **`fogspread` over the tile**
  (program slot 2 of the speckle batch, record {ALPHA 0x144, TEST 0x51011, flags 0x50} 551422–551426, fog colour `DAT_006ea970` × alpha 551429–551442,
  fog matrix `FUN_004a84a0` 551444). The fog pass is a geometry pass with the fog colour, so it fogs the whole far tile, not just the speckle. [V]
- Per map (disc `fx__` + re-fog §6): **YAV1** authors `DisableLowRes` → NO far water; near water ends at `NearSceneRange.c` = 100 m, unfogged. **KAS2 (BF1)**:
  far water to 1350 m, fogged red 1000..2992 (≤ 12 % at 1350) — the walkabout's "white bar above the red line" is the port fogging the near water AND the
  far water with the port's own ramp; the PS2 shows unfogged water to 100 m, then 12 %-red water to 1350 m, then nothing (far target). **KAM1 (BF1+BF2)**:
  `OceanEnable(1)` → ocean path, near + far to 0.8·800 = 640 m with the fog pass (551047: programs main, foam, fogspread; 551072–551076 ALPHA 0x144, TEST 0x50010).
  The port's KAM1 sea runs to its far plane and clears black behind; the PS2's ends at 640 m, ≤ 25 % black. [V code, D look]
- BF2 difference: the far speckle+fog sub-pass runs only if `SpecularEnable` is authored (708831; the near speckle likewise, 708897); no BF2 water map authors it (§5) → **BF2's far water is NEVER
  fogged and has no speckle**; BF2's far main layer is unfogged like BF1's. [V]
  **CORRECTED 2026-09-25 (`analysis/re-water-flags.md`, queue WSE's [RE] half, `[O]` UNAUDITED): the gate is right, its
  DEFAULT backwards.** `DAT_006615f9` is set to 1 by the Water effect's level reset (`FUN_005601a8`, ELF 0x56027c,
  B2:695896) before ScriptInit reads the map, and the parser overwrites it only when the key is present — which no disc
  file is — so BF2's flat water draws the near speckle on all six maps and runs the far speckle + `fogspread` sub-pass
  wherever it has a far pass. On the shipped maps the far fog shows nothing (KAS2's fog is off; NAB2/DEA1/FEL1 have no
  far layer; YAV1/DAG1 author `DisableLowRes`) — `re-water-flags.md` §0/§5.

## 2. The level
- The PS2 surface height = `LAYR` f32 (B1 538743 → `DAT_00564908`; B2 696356 → `DAT_00661604`) **+ a 6 cm bob** `0.06·sin(1.885·t)` (538346–538347, per frame
  via 135276) + `DAT_00564900` (= 0 in flat mode, 538350; the FFT min height in ocean mode 539732–539744). Tile y at 539266; flat-tile template y = 0 of the
  box (`FUN_00489f50` 550483 *(decompiler detail omitted)*, box y range [h+bob, h+bob+1] 539328); every `LPTC` vertex dequantises to the same h (550158–550164). [V]
- Per-`LAYR`, not per-tile, not from `INFO`: only the FIRST `LAYR` is read (538739–538740 *(decompiler detail omitted)*), INFO carries no height. [V]
- **The port reads the right field**: `mungefs/src/water.rs:201` `height: le::f32_at(rest, 4)` = the float after the u32, exactly what 538743 reads;
  `water_glb.rs:105/116` bakes at it; disc KAM1 −8.0, KAS2 −5.0 (BF2 KAS2 +1.0), YAV1 −72.5, KAS1 −45, NAB2 −54 (BF2 +5), DEA1 41.75, FEL1 9.895, DAG1 1.0;
  manifests `bf1_yav1/manifest.ron:4578` −72.5, `bf1_kas2:3980` −5.0, `bf1_kam1:2911` −8.0 agree. [V]
- So the YAV1/KAS2 "disagreement" is NOT the water field. What the disc says [V]: YAV1's 15 wet cells (x −32..256, z −64..32) CONTAIN the bazaar
  (`yav_bldg_bazaar` (109,−47) y −69.95, CP `bazaar` y −72.22, `yav_bldg_column`s y −72.55, `yav_prop_mist` −72.6, stumps/logs −73.0..−73.5) — an authored
  pool with mist and stumps IN it, 28 cm under the CP marker; KAS2's 128 m cells hold the piers/docks at −9.0 (decks above −5) and bamboo at −9..−5.2. The PS2
  draws the same plane there. [H] The look differs, not the level: on the PS2 the water is dark (Min/MaxDiffuse ≤ 128/255, §3), KAS2's blended at 188/255,
  and any "crates sunk to their lids" must be the port's floor/terrain height at that spot, not the plane — a PCSX2 frame at YAV1 cp5 / KAS2 cp6 settles it.
  **Corrected 2026-09-23 (cleanse):** inconsistent with §3's own `As = At × Av >> 7` a few lines below — with
  `kas2_water`'s texel alpha at 96 the product is ≈ 0.28, not 188/255; the 188 is Av alone. (RE_LOG's copy of
  this line is the same claim.)

## 3. The surface (flat-water maps; KAM1 is the ocean, below)
- **Texture**: `MainTexture` from `fx__` (`DAT_0056494c`, key 0x35a83d43, 538942), NOT `LAYR`'s name (read into a dead 256 B stack buffer 538741). UV =
  world XZ × `Tile`×INFO5 per cell, scrolled by `−Velocity/INFO5 · t` (texture matrix 551310–551313, `FUN_0039da50` 551314); i.e. metres per repeat =
  cell / (Tile·INFO5): KAS2 128/(2·4) = 16 m, YAV1 32/2 = 16 m, KAS1 32/4 = 8 m, NAB2 32/2 = 16 m. `water_glb.rs:71–78` uses `LAYR`'s pair instead
  (YAV1/KAS1/NAB2 agree by luck; KAS2 LAYR (1,1) vs fx Tile(2,2)·4). [V]
- **Colour and alpha**: per vertex on VU1 from the 16-entry LUT (§ cit. 7): entries 0..5 Border→Min, 6..15 Min→Max (551946–551963); each vertex carries a
  random word `phase<<8 | index`, index ∈ [5,16) (`FUN_00489f50` 550499–550500, `FUN_00488f20` 550101–550107 for patches; seed 0x9733...) — a static
  Min..Max noise pattern per tile, animated by the phase nibble on VU (microcode not read) [V words, D use]. Vertex alpha from the Min.a→Max.a table
  (551893–551897) → As of ALPHA 0x44: KAS2 188, KAS1 80, NAB2 128, DEA1 100..200 = translucent water over the terrain; YAV1/KAM1 255 = opaque. [V table, D blend]
  **Corrected 2026-09-25 (§3c, `re-water-colour.md` §1.5, `[O]`):** entries 6..15 run Min → 2·Max − Min (entry 10 =
  Max), so the vertices' mean colour is MaxDiffuse, not the midpoint; and the overlay's routine B adds `SpecularColor`·F
  to both the colour and the alpha (the microcode is read now).
  **AMENDED 2026-09-17 (`analysis/re-water-alpha.md`, queue WA's [RE] half, `[O]` UNAUDITED): that vertex alpha is only HALF of As.**
  The water's TEX0 carries TCC=1/TFX=MODULATE, so `As = At × Av >> 7` with At the SHEET's own alpha — and 11 of the 12
  `MainTexture`s carry real alpha on the disc (YAV1's `yav_water` is 96 flat, not opaque). The near cross-faded SPECKLE is the
  one layer whose TCC is cleared (BF1 551846), so its As is the vertex alpha alone.
  **Corrected 2026-09-23 (cleanse):** TEN of the twelve — the census grouped the two KAM1 ocean bases
  (`kam1_water_2`) into one row (`re-water-alpha.md` §4, corrected at WA's landing).
  Authored: Min(32,40,32) Max(128,130,128) Border(160,160,160) on every map but BF2 KAS2 (30/70/10) and DEA1 (70,50,40 / 85,60,50) — the water is ≤ half-bright.
  **THE LUT SCALE IS `/255` — lead re-read 2026-09-09 night (session 19, queue item W2):** the LUT builder (`FUN_0048d068` 551904–551970, the entries at
  551945–551960) lerps each authored Border/Min/Max colour byte and stores it as `byte_lerp * 0.003921569 + 65536.0` — the PS2 float-to-fixed trick: for a
  float in [65536, 131072) the mantissa's unit is 1/128, so the low mantissa bits hold `round(byte/255 × 128)`, i.e. the GS's own 0..0x80 scale where
  255 → 128 = 1.0. The alpha table (`FUN_0048cec8` 551854–551903, the packs at 551896–551898) packs Min.a/Max.a the same way (188 → 94 = 0.74). So an
  authored 80 modulates the texture by 0.31 and the water IS dark by the disc's own numbers — HOLD 10(f)'s tint clause is closed and the port uses `/255`
  (queue item W2, `5e047c2`; its midpoint-of-the-LUT simplification stays PROVISIONAL — the faithful form is the per-vertex random index). [V]
- **Speckle layer** (both passes): `SpeckleTextures` frames (25 × `water_specularmask_NN` at 4 fps on KAS1/KAS2/NAB2/DEA1/FEL1, 2 fps YAV1; 4 × `water_specles`
  KAM1), near = two frames cross-faded (551452–551455, weight ×128), tiled `SpeckleTile` and scrolled `SpeckleScrollSpeed` (551399–551402), additive 0x48,
  colours `SpeckleSpecularColor`/`SpeckleAmbientColor` and the light vector in the constant block. The frames ARE on the discs and in the bakes (KAS2/YAV1/KAS1/
  NAB2 manifests list 25 each; KAM1 lists none of `water_specles0..3`). [V]
- **Mesh**: one (PatchDivisions·INFO5+1)² grid per wet cell (`FUN_00489f50` from 550242), far LOD = PatchDivisions/LODDecimation+1 (550243); wave displacement
  on VU [D]. `LAYR`'s u32 (10), two pairs and colour word: ZeroEditor's copy of Tile/Velocity/colour, read and dropped by both ELFs (§ cit. 1, 8) — HOLD
  10(f)'s byte-order question is moot. `WMAP` bit 3 = "this cell has an `LPTC` in `CPAT`" (§4). INFO5 = cell-size multiplier (KAS2's 128 m cells). [V]
- **KAM1 = `OceanEnable(1)`** [V disc]: `FUN_0048b280` 550926–551180 draws base `MainTexture('kam1_water_2')` Tile 4 OPAQUE (record 0x69c3c0/3d0 = 0x4100, Z-written)
  + `FoamTexture('kam1_water')` FoamTile 5 blended (0x69c3e0 = ALPHA 0x44) + far fog; the mesh is a 16×16 Tessendorf FFT per 160 m cell: Phillips spectrum
  `FUN_00477b98` 539609–539636 (A = `PhillipsConstant` 1e-5, L = V²/g with `WindSpeed` 25 → 0.1019·V², cos² on `WindDirection`, damping l = L/75 at 539630),
  ω = √(g·k) 539602, per-frame iFFT `FUN_004cea50` 539584 → mesh 539585. **CORRECTED 2026-09-17 by §3a (queue OCA): that transform is NOT 1/NM-normalised —
  the divide is gated on a 4th argument the water passes as ZERO, and the amplitude that follows is metre-scale, not the centimetres the port ships.**
  **The port paints KAM1 with the
  FOAM texture** (`bf1_kam1/manifest.ron:2910` `kam1_water`; `kam1_water_2` is baked, line 44) and no waves — the "flat grey plane". [V]
  **Corrected 2026-09-23 (cleanse):** overtaken — since W1 the `fx__` `MainTexture` outranks LAYR (the ocean
  water.glb names `kam1_water_2`), OC added the waves, and the tiling is `cell/(Tile·INFO5)` (`bake::water_glb`).

# OPUS RE (loop lead, 2026-09-17, lane 1) - [O] UNAUDITED: no Fable pass
## 3a. How the PS2 scales its ocean — the `h̃0` amplitude and the inverse transform (queue item OCA)

**The answer in one line.** Nothing between the spectrum and the vertex multiplies the height: the inverse transform the
ocean calls is **UNNORMALISED** (the `1/(N·M)` divide exists but is gated on a flag the water passes as 0), the mesh writer
hands the VU a **unit cube** — `y = (h − min)/(max − min)` — and the per-tile record carries the world box
`[base+bob+min … base+bob+max]`, so **the drawn wave height is exactly the raw transform's own peak-to-trough range**.
For KAM1's authored constants that is a **median 19.2 m peak to trough, 4.15 m rms** — not the 7.5 cm / 1.6 cm the port's
`1/(N·M)` gives. §8's "metre-scale [D]" was right and §3's "1/NM" was wrong.
**Corrected 2026-09-23 (cleanse):** "port today" here means before queue OCB — OCB landed 2026-09-17 (`efab925`)
and the port now runs the transform with NO `1/n²` (`game/src/water_anim.rs`, `Ocean::inverse`).

**The hops, both discs, each with its enclosing function** (BF1 = `SWBFRONT.ELF-decomp.c`, BF2 = `SWBFRNT2.ELF-decomp.c`):

| what | BF1 | BF2 |
| --- | --- | --- |
| ocean tick, gated on `OceanEnable` | `FUN_00475658` 538323–538325 (`DAT_005647b8 == 1`) | `FUN_005603e0` 695967–695969 (`DAT_00661490 == 1`) |
| `h̃0` build (once, at load) | `FUN_00477600` 539440–539497 | `FUN_005620d0` 697009–697058 |
| Phillips `P(k)` | `FUN_00477b98` 539607–539636 | `FUN_00562628` 697165–697195 |
| `ω = √(g·k)` | `FUN_00477b60` 539594–539606 | `FUN_005625f0` |
| per-frame `h̃(k,t)`, iFFT, mesh | `FUN_00477928` 539522–539592 | `FUN_005623d0` 697086–697149 |
| 2-D inverse transform | `FUN_004cea50` 593638–593672 | `FUN_005c4488` |
| 1-D butterfly (NR `four1`) | `FUN_004cece8` 593674–593762 | `FUN_005c4558` |
| mesh writer (the unit cube) | `FUN_00477cd0` 539639–539988 | `FUN_00562760` 697197–697490 |
| per-tile record (the world box) | `FUN_004770a8` 539201–539341 | `FUN_00561b58` 696781–696915 |
| packet builder (u16 vertices) | `FUN_0048a6c8` 550630–550781 | `FUN_00574a70` 707985–708100 |

**(b) The normalisation — the divide is OPTIONAL and the ocean opts out.** `FUN_004cea50(buf, N, M, scale)` runs the
transform and then, **only if `scale != 0`**, divides every element by `N·M` (593649–593668). The direction is not that
argument: inside the wrapper it is the hardcoded immediate `+1` in the `jal`'s delay slot — ELF `004cea78: 0c133bd8 jal
0x004cef60` / `004cea7c: 24070001 addiu a3,zero,1`, against the forward wrapper `FUN_004cea30`'s
`004cea3c: 2407ffff addiu a3,zero,-1`. The water's call site sets the flag to zero in the bytes:
`00477b00: 0000382d daddu a3,zero,zero` → `00477b04: 0c133a94 jal 0x004cea50`. The 1-D routine itself scales nothing
(593674–593762 is Numerical Recipes' `four1`: bit-reversal, then butterflies on the `−2sin²(θ/2)` / `sin θ` recurrence).
BF2 is identical in its own bytes: `005c44b4: 24070001 addiu a3,zero,1` inside the wrapper, `00562598: 0000382d daddu
a3,zero,zero` at the water's call. **[V] [O]**

**(a) What scales `h̃0` between the spectrum and the vertex: nothing, and the mesh carries no metres at all.**
`FUN_00477cd0` scans the REAL parts of the transform's output for a minimum and a maximum (539732–539748, sentinels
`±100000.0`, ELF `0x47c35000` / `0xc7c35000` at `00477dac`/`00477dbc`) and then writes each of the `(N+1)²` vertices as a
**unit cube**: `y = (h − min)/(max − min)`, `x = i/(N+1−1)`, `z = j/(M+1−1)` (539780–539782). `FUN_0048a6c8` packs each
vertex as four `u16` (8 B): `x = (x·0.5 + 0.25)·65535`, `y = y·65535`, `z = (z·0.5 + 0.25)·65535`, `w` = the strip flag
(550716–550718; ELF immediates `0x3f000000` 0.5, `0x3e800000` 0.25, `0x477fff00` 65535, `0x42fe0000` 127 for the byte
normals, `0x45000000` 2048 for the UVs, moved to the FPR at `0048a7d4`–`0048a800`). The only metres in the draw are the
per-tile record's two corners, written by `FUN_004770a8` through `FUN_00478bd8`: corner 0 =
`(x0 − cell/2, base + bob + min, z0 − cell/2)` (539327) and corner 1 = corner 0 + `(2·cell, max − min, 2·cell)`
(539328–539330). `FUN_00478bd8` stores THREE floats (540231–540233; ELF `00478bd8`–`00478be4` = three `swc1` then
`jr ra`), so the 0x30-byte record is two PADDED vec3s at `rec[0..2]` and `rec[4..6]` — `rec[3]`/`rec[7]` are never written
by these calls although `FUN_0039bd98` copies all four words of each. Those corners go to the renderer as the generic
**vertex-dequantisation box** — `FUN_0039bd98(dl, rec, rec+0x10)` (551104 and 551165), a generic display-list primitive
with **57 call sites in BF1, 31 of them within thirty lines of a `FUN_00398928` mesh submission** (measured), the same
pair the ocean itself uses (its own submit is `FUN_00398928` at 550757), and the same convention the port already
implements for PS2 models (`mungefs/src/model.rs:237–250`: `pos = vbox.min + q·(vbox.max − vbox.min)/65535`). So
`world_y = base + bob + min + y_u16/65535 · (max − min)`, and the horizontal halves fit exactly: `x` lives in
`[0.25, 0.75]` of a box `2·cell` wide, i.e. exactly the cell `[x0, x0 + cell]`. **[V] for the writes and the packing,
[D] for the dequantisation (the box semantics are read off the model path, not off this VU program) [O]**

**(c) The VU1 microcode does not scale the displacement — and the ocean does not even run the water microcode.**
The flat-water renderer uploads VU program `0x515410` / `0x514ec0` (551518, 551546, both inside `FUN_0048b9c0`) and the
23-qword constant block `0x754a30` with the 16-entry Border→Min→Max LUT (551556, 551622, 551965 — the only three
appearances of that address in BF1). **The ocean renderer `FUN_0048b280` (550926–551180) references none of them**: it
submits an ordinary mesh packet through `FUN_00398928` and draws each tile with `FUN_0039b488`. Its vertex colour is
written on the EE, not on the VU: `RGB` = the renderer colour bytes `DAT_0069c420..422` halved to the GS 0..0x80 scale and
**`A` = the foam byte halved** (550727, 550744). That answers half of §8's "exact per-vertex alpha" for the OCEAN path —
the 16-entry LUT and its alpha are the FLAT-water path's only. **[V] for the absences (searches below) [O]**

**The foam byte, and why it settles the normalisation on its own.** `FUN_00477cd0` keeps a byte per mesh point
(`DAT_00564958`): when a negative second difference of the RAW height field exceeds **3.0** (ELF `0x40400000` at
`004782a4`) in either axis the byte gains 6, else it loses 4, clamped to `[0,255]` (539941–539985). That byte is the
ocean's per-vertex alpha (550727 and 550747), and KAM1 pairs an opaque base `kam1_water_2` with a blended
`FoamTexture('kam1_water')` (records `0x69c3c0`/`0x3d0` and `0x69c3e0`, §3 cit. 5) — so the byte is what makes foam
appear; the guard is at 539949–539954 and the two updates at 539955–539971. Measured on the disc's own
pipeline (`lead_tmp/re/water-amplitude/foam_check.py`, 30 frames): **unnormalised, the test fires on a median 13.3 % of the
points (8.6–17.2 %) — foam on the crests; with the port's `1/(N·M)`, it fires on 0.00 % of them, every frame.** Under the
port's normalisation the disc's entire foam mechanism would be dead code. **[D] [O]**

**The numbers** (`lead_tmp/re/water-amplitude/disc_ocean.py` + `amplitude_stats.py`, the disc's formulas re-implemented
hop for hop; the hand-written transform and numpy's agree to 2.6e-14 m). KAM1 on BOTH discs authors the same inputs —
`WATR INFO` cell **160.0 m**, INFO5 1, and `fx__ Effect("Water")` `PatchDivisions(16,16)`, `WindSpeed(25)`,
`WindDirection(0.2, 1)`, `PhillipsConstant(1e-05)`, `LODDecimation(2)`, `LAYR` height −8.0 (re-read 2026-09-17 with
`lead_tmp/s17/re/{watr,fxwater}.py`). Over 320 frames (40 seeds × 8 times):

| | disc (no `1/(N·M)`) | port today (`1/(N·M)`) |
| --- | --- | --- |
| peak-to-trough range | median **19.23 m** (p5 13.0, p95 30.3, max 41.9) | 0.0751 m |
| rms | median **4.145 m** | 0.0162 m |
| significant height 4σ | 16.6 m | 0.065 m |
| max \|h\| over 40 seeds × 160 frames | 23.51 m | 0.092 m |
| `Ocean::bound` (triangle inequality) | median 37.0 m | 0.1445 m (the OC build measured 0.1461) |

**Corrected 2026-09-23 (cleanse):** "port today" here means before queue OCB — OCB landed 2026-09-17 (`efab925`)
and the port now runs the transform with NO `1/n²` (`game/src/water_anim.rs`, `Ocean::inverse`).

The port's own measurement (1.25–1.90 cm rms, bound 0.1461 m, queue OC) reproduces in this model to three digits, which is
what makes the ×256 comparison a like-for-like one. **[V] measured [O]**

**The spectrum itself is unchanged from §3** and every constant was read as an ELF word, not off the decompiler:
`FUN_00477b98` builds `P(k) = A·exp(−1/(k²L²))·|k̂·ŵ|²/k⁴·exp(−k²(L/75)²)` with `L = WindSpeed²·0.101936795`
(immediate `0x3dd0c43d` at `00477bcc`, `0x3c5a740e` = 1/75 at `00477c60`, `0xbf800000` at `00477c3c`); `FUN_00477600`
sets each part of `h̃0` to `√P · 0.70710677 · N(0,1)` (`0x3f3504f3` at `00477724`) on the wave grid
`((i + N/2) mod N − N/2)·2π/cell` (`0x40c90fdb` at `00477714`); `ω = √(9.81·k)` (`0x411cf5c3` at `00477b80`);
`FUN_00445bd0` (505123) is a Marsaglia-polar standard normal, `FUN_00435b30` `sqrtf`, `FUN_00434110` `fabsf`
(`& 0x7fffffff`), `FUN_00435370` `expf`. `WindSpeed`/`WindDirection`/`PhillipsConstant`/cell are **zero in the ELF image**
(`0x00564944`, `0x0056493c`/`40`, `0x00564948`, `0x00564918` are .bss) — a map that authors no `Effect("Water")` values
gets no sea. **[V] [O]**

**The searches behind the absence claims** (§5 self-check 2; counts are `grep -c` over the whole corpus):
`DAT_00564900` 11 and `DAT_00564904` 8 appearances in BF1, ALL inside `FUN_004770a8`, `FUN_00477cd0` and the initialiser at
538349–538350; the pointer-offset forms `0x4900`/`0x4904` 0 hits, the decimal forms 1 hit (a function NAME, `00186888`) and
0. BF2's `DAT_006615fc` 11 / `DAT_00661600` 8, same three sites; its `0x1600` hits (21) are all other subsystems' struct
fields. **The verifier agent re-ran this one over the ELF instead of the corpus** — every load/store/`addiu`/`ori` whose
16-bit field is `0x4900` (11 instructions) or `0x4904` (9), any base register — and found the same three functions plus one
hit that resolves to `0x00534900`: the absence holds. `0x754a30` 3 hits and `0x515410`/`0x514ec0` 3 hits, none in
`FUN_0048b280`. The other two searches are **narrower than first written, corrected after the verifier falsified the
wording**: the foam map `DAT_00564958` lives in FOUR functions, not two — allocated and cleared in `FUN_00477600`
(539462, 539465), read-modify-written in place in `FUN_00477cd0` (539955–539971), freed through `FUN_004778a0` (539511) and
read into the vertex colour in `FUN_0048a6c8` (550699, 550706); the transform buffer `DAT_00564960` lives in THREE
(`FUN_00477928`, `FUN_00477cd0`, plus a pointer init at 539464 and a free-time null check at 539514 that never touch its
contents). The claim that survives both is the one that matters: **no function outside the mesh writer alters a height or a
foam byte between the transform and the packet.** **[V] [O]**

**A shipped disc bug, found while checking the above.** The foam map is allocated `(N+1)·(M+1)` bytes (539462) and then
cleared by `FUN_004266d8(DAT_00564958, 0, 1)` (539465) — `memset(ptr, byte, size)` with **size = 1**, confirmed in the ELF
(`004776e0: daddu a1,zero,zero`, `004776ec: addiu a2,zero,1` in the `jal`'s delay slot). Only byte 0 is initialised; the
other 288 start as whatever the heap held. The −4/frame decay washes that out within ≤ 64 frames, which is why nobody saw
it. A port must zero the whole map and record the deviation (queue OCF — done 2026-09-23, `42f85e2`; PLAN stated
deviation 15). **[V] [O]**

**What stays open.** The VU1 program the generic mesh path runs is still unread, so the dequantisation is [D] and not [V];
a PCSX2 GS dump of KAM1 would close it — and would also show whether the far LOD (`LODDecimation(2)`, the second packet
`FUN_0048a6c8(…, DAT_005648f0)`) is displaced the same way. The `0.5/0.25` half-cell margin in the horizontal packing is
explained by the `2·cell` box but nothing says why the box is 2 cells wide rather than 1 — a horizontal (choppy)
displacement would want exactly that margin, and that too is VU-side. **[H] [O]** HOLD 21(a) keeps the flat-water LUT,
the phase nibble and the far-LOD question; its ocean half is closed by this section.
**Corrected 2026-09-23 (cleanse):** answered in §3b — that packet is built and never issued, so there
is no displaced far LOD for the ocean (queue OCL).

**Self-checks run** (§5): (1) every citation above names its enclosing `FUN_`, found by scanning backwards
(`lead_tmp/re/water-amplitude/encl.sh`); (2) every absence claim has the search and its counts, above; (3) the BF2 twin was
opened for the whole chain and is gated on BF2's own `OceanEnable`, not on a flat-water or debug path; (4) every constant
was read as an ELF word with `off = va − 0x160000 + 0x1000` (verified from both program headers) using
`lead_tmp/re/water-amplitude/{mipsdis,litscan2}.py` (the addresses given for an immediate are the `mtc1` that moves it,
with the `lui`/`ori` pair on the two instructions before).

**The verifier agent** (one Opus pass, §5's falsification half, claims and citations only — it saw no reasoning).
SUPPORTED: the normalisation flag and the `+1`/`−1` directions in both discs' bytes, the unit cube, the packing, the tile
record, the `OceanEnable` gates (it went further and derived the FNV-1a key hashes itself: `OceanEnable` 0xa69c431a,
`PhillipsConstant` 0xd25ef90a, `WindSpeed` 0x13901a72, `WindDirection` 0x415e2b30, `PatchDivisions` 0x009036eb — the same
dispatcher), the Phillips formula and every ELF immediate, the Marsaglia normal, and the disc reads (it re-read both
`KAM1.LVL` payloads byte for byte: `INFO` is identical on the two discs, `00002043` = 160.0, and every `fx__` value is a
float32 — `PatchDivisions` is `16.0f`, `OceanEnable` is `1.0f`). NOT SUPPORTED, and corrected above: two absence claims
were too narrow as worded (the foam map and the transform buffer each have pointer-lifecycle sites I had not listed), and
four line ranges drifted by one to four lines. It also found `FUN_00478bd8`'s three-float store and the `memset(…, 1)`
bug, both folded in above. Nothing it found contradicts the amplitude result.

# OPUS RE (loop lead, 2026-09-17, lane 2) - [O] UNAUDITED: no Fable pass

## 3b. The ocean's LOD — the disc builds a decimated mesh every frame and never draws it (queue item OCL)

**The answer in one line.** The ocean DOES take a decimated grid — `FUN_0048ac90` builds its mesh twice every
frame, once at stride 1 and once at stride `LODDecimation` — but **only the stride-1 mesh is ever issued to the
GS**. The decimated mesh's address is read by exactly ONE instruction in each ELF, inside the FLAT-water
renderer, which the water dispatcher runs only when `OceanEnable == 0`. So on KAM1 the 9×9 mesh is rebuilt and
thrown away on every frame that steps the sea, and **the drawn ocean is one resolution — full `PatchDivisions` — at every
distance**. There is no near/far mesh join on the ocean path, and therefore no seam to reproduce: OCL's
option (2b) is the disc's own behaviour.

**The hops, both discs, each with its enclosing function** (BF1 = `SWBFRONT.ELF-decomp.c`, BF2 = `SWBFRNT2.ELF-decomp.c`):

| what | BF1 | BF2 |
| --- | --- | --- |
| water dispatcher (flat vs ocean) | `FUN_0048b1e0` 550901–550919 | `FUN_005755b0` 708249–708272 |
| `OceanEnable` global | `DAT_005647b8` | `DAT_00661490` |
| ocean renderer | `FUN_0048b280` 550926–551180 | `FUN_00575670` 708275–708585 |
| flat-water renderer | `FUN_0048b9c0` 551182–551524 | `FUN_00575fd8` from 708586 |
| ocean mesh writer (per frame) | `FUN_00477cd0` 539639–539988 | `FUN_00562760` 697197–697490 |
| **the two-LOD builder** | `FUN_0048ac90` 550783–550798 | `FUN_00575060` 708131–708148 |
| strip/packet builder (takes the stride) | `FUN_0048a6c8` 550630–550781 | `FUN_00574a70` 707985–708130 |
| flat-water static grid builder | `FUN_00489998` 550231–550260 | `FUN_00573d78` 707584–707761 |
| the DMA-call draw primitive | `FUN_0039b488` 387593–387637 | `FUN_00496eb0` (same `0x50000000` tag, same ADDR mask) |
| near/far range setter | `FUN_00491148` 554252–554259 | — |
| `LODDecimation` global | `DAT_005648f0` | `DAT_006615e8` |
| near mesh handle (stride 1) | `DAT_005649c0` | `DAT_006616c4` |
| far mesh handle (decimated) | `DAT_005649c4` | `DAT_006616c8` |

**(a) The ocean builds two meshes per frame, and the second one is decimated by `LODDecimation`.** The ocean's
per-frame mesh writer ends in *(decompiler detail omitted)* (BF1 539979, inside `FUN_00477cd0`; BF2 697487
*(decompiler detail omitted)* inside `FUN_00562760`). That function rotates a triple buffer, opens one packet
on it and builds the mesh **twice**:

*(decompiler excerpt omitted from the public copy)*

The 4th parameter is a grid STRIDE: inside `FUN_0048a6c8` the row index advances *(decompiler detail omitted)*
(550687) and the column index *(decompiler detail omitted)* (550739), so stride 1 walks every point and stride
`DAT_005648f0` every `LODDecimation`-th. `DAT_005648f0` is `LODDecimation`: it defaults to 2 (538252
*(decompiler detail omitted)*), is overwritten from the `fx__` config at 538965, and is the divisor that makes the FLAT
path's far grid at 550243. Both KAM1s author `OceanEnable(1)`, `PatchDivisions(16, 16)` and `LODDecimation(2)`
(read off both discs with `lead_tmp/s17/re/fxwater.py`), so the pair is a 17×17-vertex mesh and a 9×9 one.
Corroboration from a third place: `FUN_00489998`'s OCEAN branch sizes the shared packet for **both** meshes —
`(DAT_005648e8 * DAT_005648ec + (DAT_005648e0 / DAT_005648f0 + 1) * (DAT_005648e4 / DAT_005648f0 + 1)) * 3`
(550246–550247), three qwords per vertex (the GIF packet's ST / RGBAQ / XYZ2 triple). **[V] [O]**

**(b) Only the stride-1 mesh is ever drawn — and this is a byte-exhaustive absence, not a reading.**
`FUN_0039b488(dl, handle)` is the draw primitive: it appends a DMAtag whose first word is `0x50000000` — id 5,
a DMA **call** — with the ADDR field *(decompiler detail omitted)* (387615–387616). A mesh whose handle is never passed
to it is never transferred, and so never drawn.

Scanning **every** load/store/addiu word in each ELF for the handle's 16-bit offset (`off = va − 0x160000 +
0x1000`, single PT_LOAD, read from the program header of both files; script `lead_tmp/re/ocean-lod/scan_glob.py`):

| handle | writes | reads | the only read's function |
| --- | --- | --- | --- |
| BF1 far `DAT_005649c4` | `00489a1c`, `0048ad24` | **`0048baac: 8c5449c4 lw s4,0x49c4(v0)`** | `FUN_0048b9c0` — FLAT |
| BF1 near `DAT_005649c0` | `004899f8`, `0048ad04` | `0048b5d0`, `0048b940` (ocean), `0048baa4` (flat) | `FUN_0048b280` + `FUN_0048b9c0` |
| BF2 far `DAT_006616c8` | `00573dfc`, `005750f4` | **`005760dc: 8c5616c8 lw s6,0x16c8(v0)`** | `FUN_00575fd8` — FLAT |
| BF2 near `DAT_006616c4` | `00573dd8`, `005750d4` | `00575acc`, `00575f54` (ocean), `005760d4` (flat) | `FUN_00575670` + `FUN_00575fd8` |

The decompiler's own counts agree exactly (BF1 `DAT_005649c4` 3 occurrences, `DAT_005649c0` 5; BF2
`DAT_006616c8` 3, `DAT_006616c4` 5), and the ocean renderer's full 464-instruction body (BF1 0x48b280–0x48b9bc)
contains no reference to the far handle at all. **So the far handle is read only by the flat renderer — and the
dispatcher runs the flat renderer only when `OceanEnable == 0`:**

*(decompiler excerpt omitted from the public copy)*

and on an ocean map `FUN_00489998`'s flat branch never runs either, so `DAT_005649c0` can only hold the
stride-1 OCEAN mesh (the ELF has exactly two writers of it, listed above). **[V] [O]**

One nuance, so the gate is not over-read: `DAT_005647b8` has THREE states, not two. The dispatcher branches
`== 0` (flat) / else (ocean), but the ocean TICK that builds the meshes runs only on `== 1`
(`FUN_00475658` 538323–538325, `FUN_00477928()`), and `== 2` is a third mode handled at 538328. Both KAM1s
author `OceanEnable(1)` (§4 re-read the payload: every `fx__` value is a float32 and this one is `1.0f`), so
the path read here is the shipped one; what mode 2 does is unread and not needed. **[V] [O]**

**The decompiler drops the argument at the ocean's own draw call, and the bytes put it back.** The decompiled
ocean loop reads *(decompiler detail omitted)* (551113, 551169) — one argument — while the flat renderer's reads
*(decompiler detail omitted)*. The bytes show the ocean passes the near handle too; Ghidra lost it because the
load also feeds the branch:

```
0048b5d0: 8ec549c0  lw a1,0x49c0(s6)      # a1 = the stride-1 handle
0048b5d4: 10a00005  beq a1,zero,0x0048b5ec
0048b5d8: 00000000  nop                   # a1 untouched
0048b5dc: 0c0e6d22  jal 0x0039b488
0048b5e0: 0200202d  daddu a0,s0,zero      # delay slot sets a0 only
```

identically at `0048b940`–`0048b950`, and in BF2 at `00575ac8` (`lui v0,0x0066` + `lw a1,5828(v0)`, which also
pins the base as 0x660000) and `00575f50`. **[V] [O]**

**The two loops side by side — the ocean's is the flat one with the LOD choice removed.** This is the same
pair of calls §4 reads as the `CPAT` either/or, and putting the two renderers' tile loops next to each other is
the clearest single piece of evidence:

*(decompiler excerpt omitted from the public copy)*

`a local` is the flat path's LOD-selected handle (551284–551286: far when the pass argument is 0, near when it is
not). The ocean's loop is that loop with `a local` replaced by the stride-1 handle outright and the `CPAT`
either/or dropped — so `FUN_0039cdc8` is the no-mesh fallback in both, not a second mesh, and on an ocean map
it is unreachable anyway (`FUN_0048ac90` rewrites `DAT_005649c0` every frame). Note the decompiler drops the
argument in the `CPAT` arm too (`…` 551359, where the handle is `tile[+0x24]`) — the same
Ghidra failure as the ocean's, which is why §3b reads the bytes rather than the C. **[V] [O]**

**(c) The ocean's two passes are two RANGES, not two resolutions.** Both renderers take the same argument from
the dispatcher — `((argument 1 >> 17) ^ 1) & 1` — and in the flat renderer that argument is exactly what picks the
mesh (551284–551286: far when 0, near when non-zero). The ocean renderer takes the same argument and runs two
almost identical bodies that differ in the material record they bind (`0x69c3d0` + foam `0x69c3e0` on the far
pass, `0x69c3c0` + foam on the near one) and in the range it sets before rebuilding the per-tile record array —
but **both bodies draw every tile with the same stride-1 mesh**. The far pass calls
`FUN_00491148(FUN_00479d90(), 0.8 * FUN_00479db0(), ctx)`, which stores the pair at `ctx+0x80`/`ctx+0x84`
(554254–554255); the getters return `DAT_00677660` (default `0x42900000` = 72.0, i.e. `NearSceneRange[1]`) and
`DAT_00677668` (default `0x44fa0000` = **2000.0**, `FarSceneRange`). The multiplier is an inline immediate,
`0048b624: lui at,0x3f4c` + `0048b628: ori at,at,0xcccd` = `0x3f4ccccd` = **0.8**, against the flat renderer's
`DAT_00522a7c`, whose ELF word is `0x3f666666` = **0.9**. That 0.8/0.9 split is already the port's
(`viewer.rs:31–32` `FAR_WATER_FRACTION` / `OCEAN_FAR_FRACTION`), cited there from the decomp; this RE re-read
both out of the file bytes instead — the 0.8 as the `lui`/`ori` immediate pair that builds it, the 0.9 as a data
word — and they hold. **[V] [O]**

**What it means for the port — DONE 2026-09-17, landed as `6bf0d60` (queue OCL's build half, lane 2; report
`engine/artifacts/loop/reports/20260917_2110_build_land_OCL.md`).** `game/src/water_anim.rs` used to build and
upload two patches per frame and pick between them per cell — `let mesh = if d <= o.ranges.near { hn } else
{ hf };` — which is what opened the crack. It now builds ONE, and `OceanDraw::mesh` is a single
`Option<MeshHandle>` so there is nothing left to pick between; `ranges.near` is kept for what the disc
actually uses it for (the pass split) and reads nothing today.

**The crack was 3.229 m, not the 2.27 m this section first carried.** That figure was the pre-OCB 8.9 mm
scaled by the amplitude ratio and was never re-measured after OCB landed. Measured directly on
`Ocean::patch` at `every` = 1 vs 2, along both shared cell edges, over 300 frames at 60 Hz: worst parting
**3.2292 m** at t = 1.900 s (the field 21.85 m peak-to-trough at that instant), mean per-frame worst
**2.4262 m** — so 2.27 m read as typical, not worst.

At `PatchDivisions(16,16)` / `LODDecimation(2)` dropping the decimation raises a far cell from 128 to 512
triangles (81 → 289 vertices) — 384 more triangles per far cell, not the 192 the queue entry estimated,
which counted quads. Measured on bf2_kam1 over 8 `water_frames` runs a side, both directions: the step alone
falls **0.0127 → 0.0112 ms** median (one patch built and uploaded instead of two) and the extra geometry is
invisible end to end (moving-frame median 0.175 → 0.168 ms), with the draw count identical either way.
Whether the port should keep paying for the decimated mesh the disc builds and discards was a port decision,
not a fidelity one — nothing can observe it — and it was decided AGAINST porting it.

**Five things the verifier added, and they sharpen rather than weaken the result.** (1) **What selects the
far pass is bit 17 of the render flags** — both renderers are called as `((argument 1 >> 17) ^ 1) & 1`, so the
decimated mesh is reachable only from the `FUN_00475940(0x20000)` / `FUN_00560540(0x20000)` call site, and the
dispatcher gates that path additionally on `DAT_005229c4 != 0`, which defaults to 1 (538254) and is zeroed by
the fx key `0xa01d9243` = FNV-1a("disablelowres") — which KAM1 does not author. (2) **`DAT_005647b8` is a
tri-state written by TWO keys**, not one: `0xa69c431a` = FNV-1a("oceanenable") writes 0/1 (538984) and a
second, unidentified key `0xa015e2ff` writes **2** (538971/538974); the dispatch claim (`== 0` flat, `!= 0`
ocean) is unaffected, but "OceanEnable" is an incomplete label for the global. (3) **BF2's dispatcher carries a
guard BF1 does not** — its ocean arm is `else if (DAT_0060beea != '\0')` where BF1's is a bare `else`, so the
twins are structurally equal at every hop that matters here but not literally identical. (4) **`LODDecimation`
is CLAMPED, not validated**: `if (DAT_005648e0 < DAT_005648f0) …` (538836, 538885)
caps it to the *unscaled* `PatchDivisions` — where the port instead REJECTS a `LODDecimation` that does not
divide `PatchDivisions`. Moot once the port stops using the field, and recorded so the build can delete that
check knowingly. (5) **None of the `fx__` key names is a string in either ELF** (`grep -aob "cean"` → 0 hits
in both): every key identification here and in §3/§5 rests on the FNV-1a-of-lowercase hash, which the verifier
re-derived and cross-checked against six unrelated keys in the same parser. Read the key names as [D], not [V].

One precision on the offset formula: `off = va − 0x160000 + 0x1000` holds only for `va < 0x160000 + FileSiz`
(BF1 FileSiz 0x4043c8, so below 0x5643c8). The mesh handles at 0x5649c0/0x5649c4 are **past** it — they live in
`.sbss` and are correctly absent from the file image, which is why they are found by scanning instructions
rather than by reading a word. `DAT_00522a7c` (0.9) is inside the image and WAS read as a file word; the
72.0/120.0/2000.0 figures are the immediates the code stores (540538–540541), not file words, and they are
defaults — `DAT_00677668` has a setter reached from the fx key `0x1192b7b` = FNV-1a("farscenerange"), which is
what makes the `FarSceneRange` identification solid rather than a guess from the value.

**Verification.** One Opus verifier agent, given the claim list and the citations only, never the reasoning
(`lead_tmp/re/ocean-lod/claims.md`). It re-read every citation, named every enclosing function, and attacked
the three absence claims with a **stronger** search than the one they cite: instead of matching the 16-bit
immediate it tracked `lui`/`addiu`/`ori` values to compute each memory op's real effective address over all
967,919 `.text` instructions, covered `ld/sd/lq/sq` as 8- and 16-byte spans, checked the `$gp` path (`.reginfo`
`ri_gp_value = 0`, so there is none), searched the data sections for a literal pointer to the handle (0 hits)
and — the vector that beats value tracking — scanned all of `.text` for any `lui`+`addiu`/`ori` chain that
could *form* the address in a register (0 results, so no indirect or argument-passed read can exist). All three
absence claims survived. It also walked both renderers' caller chains to the two flag call sites to confirm the
BF2 pair is the real twin and not a debug or reflection variant, and disassembled `FUN_00575060` against
`FUN_0048ac90` to find the same 32-instruction sequence differing only in the global base. **Every technical
assertion was SUPPORTED; what it broke was four CITATIONS** — three already corrected here before it reported
(550661→550687, 551110→551113, 708487→**697487**, the last of which pointed at `FUN_00575670`, the flat-side
neighbour, and is exactly the confusion self-check 1 exists to catch) and one it caught that had not been:
`DAT_00522a7c * a local` is at 551292, not 551290. All four are fixed above.

**Self-checks run** (`OPUS_LOOP_PROMPT.md` §5): (1) every citation's enclosing `FUN_` printed by awk and named
above — the ocean/flat pair is the exact hazard the 2026-09-14 trial failed, so the dispatcher's `OceanEnable`
branch is quoted in full rather than asserted; (2) the absence claims rest on a full-ELF instruction scan on
BOTH discs, with the decompiler's counts as an independent cross-check, not on a reading; (3) the BF2 twin was
opened at every hop — builder, renderer, dispatcher and draw call — and its ocean renderer reached through the
same `OceanEnable` gate, not an alternate or debug path; (4) every constant read out of the file — as a data word where it
lives in the image, as an instruction immediate where it does not — with the offset formula stated and the
program header re-read to derive it for BF1 as well as BF2.

**What stays [H]:** that the two meshes live in one packet as separately-callable sub-lists (the reading of
`FUN_0039b638` / `FUN_0039b350` as open/close of a sub-list) is [D], not [V] — but the load-bearing claim does
not need it, since the far handle is never passed to the DMA-call primitive on the ocean path whatever the
packet's shape. Why the disc builds a mesh it never draws is unread: the likeliest reading is that the ocean
path inherited the flat path's builder wholesale, and nobody deleted the second call. Nothing here touches the
VU1 microcode, which remains HOLD 21's unread half.

# OPUS RE (loop lead, 2026-09-25, lane 1) - [O] UNAUDITED: no Fable pass
## 3c. The flat water's per-vertex colour — the VU1 overlay read (queue WCOL; the full RE is `analysis/re-water-colour.md`)
The flat-water "microcode 0x515410" is a DMA chain that uploads a 63-instruction VU1 overlay (MPG at VU 0x74e; BF2
0x5f6660 → 0x758, word-identical). Its routine B colours the MAIN layer per vertex, UNLIT: colour = LUT[i] +
`SpecularColor`·F, alpha = ALPHA[phase] + `SpecularColor.a`·F, F = (1 − max(0, V̂.y))³ with V from the vertex to the eye
shifted 10 m along the light's horizontal direction — a view-angle term the port lacks, and on the PS2 (patched save
states) the source of the water's teal body (+13..+27 G/B levels on BF1 KAS2, NAB2, BF2 FEL1). The LUT runs Min →
2·Max − Min over the vertex indices 5..15, so its mean is MaxDiffuse, not §3's midpoint; routine A colours the speckle
`SpeckleAmbient + SpeckleSpecular · lobe³²` around the sun's azimuth, not flat; the ocean's vertex RGB is the engine's
global white halved = unity. [V][O] — `re-water-colour.md` §0–§3; one Opus verifier, 11 SUPPORTED / 2 PARTLY (corrected).

## 3d. The ocean foam's texture matrix — `FoamTile` (issue #18, card FOAMT, 2026-10-28) [O]

- **Load.** The `fx__ Effect("Water")` loader stores `FoamTile` (fnv 0xdcb91b88) raw: B1 `FUN_004763d0` 538988–538990
  (`DAT_005648f8` = x, `DAT_005648fc` = z; reset to 1.0 at 538253 / 538259); B2 696576–696578 (`DAT_006615f0` / `f4`;
  reset 695895 / 695901). The finalise (538831–538834, its twin 538880–538883) multiplies only the BASE `Tile`
  (`DAT_00564934` / `38`) by INFO5 (`DAT_00564920`) and divides `Velocity` by it; `FoamTile` is never scaled (its only
  other readers are the draw below: `re_check.py writers b1 DAT_005648f8`).
- **Draw.** The ocean pass B1 `FUN_0048b280` builds two texture matrices from one stack record (551026–551035): the
  base with diagonal (`DAT_00564934`, `DAT_00564938`) and translation the scroll's fraction
  `t·(−Velocity/INFO5) − ⌊…⌋`, sent by `FUN_0039da50(0, a local, …, 0x1800, 0)`; then the same record with only the
  diagonal replaced by (`DAT_005648f8`, `DAT_005648fc`) — the foam's, sent to `a local`. Same scroll offset, own
  scale. B2 the same at 708397–708398.
- **So** metres per repeat: base `cell / (Tile·INFO5)`, foam `cell / FoamTile`. Both KAM1s (`INFO` cell 160.0, INFO5 1,
  `Tile(4,4)`, `FoamTile(5,5)`): base 40 m, foam 32 m. The port reused the base's UVs for the foam (40 m) until this card;
  now `ManifestWaterConfig::foam_metres_per_repeat` and the foam sheet's `uv_scale` (base / foam = 1.25) carry it.

## 4. `CPAT` REPLACES
- Parser: for every cell whose nibble has bit 3, one `LPTC` is read in row-major order and linked with (x, z) (538764–538787; B2 696379–696396). Disc: bit-3
  cells == `LPTC` count on all 14 chunks (BF1 NAB2 4 = 4, BF2 NAB2 8 = 8, BF2 DEA1 1 = 1, eleven 0 = 0); nibbles are only 0/1/9. [V]
- Cull: the tile record takes the patch packet at +0x24 when its (x, z) matches (539306–539324). Draw: `tile+0x24 != 0 → FUN_0039b488(patch)` ELSE
  `FUN_0039cdc8` (the flat template) — an either/or in every sub-pass (551350–551359, 551383–551392, 551499–551508). **A patch replaces its cell's flat quad**;
  `water_glb.rs:13–17/82–89` is right and can drop PROVISIONAL. Note the PS2's wet test is nibble ≠ 0 (539348–539355), the port's bit 0 — equal on every disc. [V]
  **CORRECTION (loop lead, 2026-09-17, lane 1 — queue item WP, [O] for the re-read, [V] for the measurement):** the RULE above is right, but "`water_glb.rs` is
  right" was too kind to the PORT. The match at 539306–539324 is an EXACT INDEX equality — `a local = z*grid + x` against `a local[1]*grid + *a local`, and `a local`
  (which becomes `a local[9]` = tile+0x24) stays 0.0 unless it breaks on equality — inside `FUN_004770a8`; the BF2 twin is `FUN_00561b58` 696884–696892, the same
  shape on `DAT_00661608`. The port instead dropped every wet cell whose CENTRE fell inside a patch's AABB, and each `LPTC INFO` box is its own cell padded by
  half a cell on every side (MEASURED, exact, on all 13 shipped patches — `bake/tests/suite/water_patch_cells.rs` asserts box == cell ± half, which also proves the
  i-th `LPTC` ↔ i-th bit-3 cell pairing geometrically), so the box reaches the eight neighbours' centres: **17 / 32 / 2 cells were removed where the disc marks
  4 / 8 / 1** (BF1 NAB2 / BF2 NAB2 / BF2 DEA1 — DEA1 drew no lattice cell at all), holes in the water beside every patch. Fixed by `Water::has_patch` (`WMAP`
  bit 3). Also measured there: a patch's triangles cover 13–96 % of the cell they replace, i.e. the "3–24 %" of §4's tile is that area over the twice-as-wide box.
  **Three amendments the same pass owes the TEXT above** (an Opus verifier agent was briefed with the citations only and asked to falsify; it did not support §4's "either/or",
  and the lead re-read every line below itself — [O][V]): (1) **tile+0x24 holds a DMA PACKET POINTER, not a float**: Ghidra types it `float` at the store (539314/539324)
  and `int` at the load (551350); provenance is the `LPTC` loader's `+0x14` (`FUN_00488f20` 550180, the chunk tag test at 550024) → record+8 (538772) → tile+0x24.
  (2) **the draw is a THREE-WAY, not an either/or** — in `FUN_0048b9c0` (551345–551358, lead-read): `+0x24 != 0` draws the patch, else `a local == 0 ? FUN_0039cdc8` (the flat
  template) `: FUN_0039b488(a local, a local)` where `a local` is `DAT_005649c0`/`DAT_005649c4` picked by the caller's flag (551284–551286). The patch test is exactly as §4 says;
  the ELSE has a middle case §4 dropped. (3) **the ocean twin never reads +0x24 at all**: `FUN_0048b1e0` (550906–550917) routes to `FUN_0048b280` whenever `DAT_005647b8`
  (= `OceanEnable`, 0xa69c431a, set at 538984 — as §1 already noted) is non-zero, and that function's whole body (550922–551180) contains NO `0x24` reference; it branches
  on `DAT_005649c0` alone (551109, 551165). So "a patch replaces its cell" holds on the FLAT-water path only, and "an either/or in every sub-pass" is too strong. No shipped
  ocean map ships a `CPAT` (both KAM1s: 0 patches, pinned by `bake/tests/suite/water_patch_cells.rs`), so nothing in the port turns on it. Also from the verifier, and checked:
  the exact-match walk is a MERGE-JOIN over a cursor set once (539251) and only ever advanced (539311) — it is correct because the parser appends the `LPTC`s in row-major
  order (538776–538781), so "one patch per cell" is a property of the BUILDER as much as of the cull; and past +0x24 the two games' tile records differ (BF1 floats at
  +0x28/+0x2c and tests +0x2c at 551343; BF2 bytes at +0x28/+0x29 and tests +0x29 at 708770), which is why only +0x24 may be quoted as common. [O]

## 5. `fx__ Effect("Water")` census (disc, `fxwater.py`; all 12 water maps; the three colour keys 0x3e68131d/0x399cb557/0x19fb4b6e (148,170,192 / 192,192,192 ×2)
and 0xbf2b02c2(4) are authored but consumed by neither ELF — PC keys [V unconsumed, H PC])
| map | Ocean | LowRes | Tile | Velocity | PatchDiv | LOD | MainTexture | Min/Max/Border alpha | Speckle (tile, scroll, frames@fps) |
|---|---|---|---|---|---|---|---|---|---|
| BF1 KAM1 | **1** | far on | 4,4 (+Foam 5,5 `kam1_water`) | .01,.05 | 16,16 | 2 | `kam1_water_2` | 255 | 5,5 / 0,0 / specles0-3 |
| BF1 KAS1 | 0 | **DisableLowRes** | 4,4 | .02,.02 | 3,3 | 8 | kas1_water | **80** | 10,10 / .03 / 25@4 |
| BF1 KAS2 | 0 | far on (INFO5 = 4) | 2,2 | .05,.05 | 4,4 | 8 | kas2_water | **188** | 5,5 / .11 / 25@4 |
| BF1 NAB2 | 0 | far on | 2,2 | .05,.05 | 3,3 | 3 | nab2_water | **128** | 2,2 / .02 / 25@4 |
| BF1 YAV1 | 0 | **DisableLowRes** | 2,2 | .1,.02 | 8,8 | 8 | yav_water | 255 | 4,4 / .2,0 / 25@2 |
| BF2 KAM1 | **1** | far on | as BF1 | | 16,16 | 2 | kam1_water_2 | 255 | (no SpecularEnable) |
| BF2 KAS2 | 0 | far on (INFO5 = 4) | 1,1 | 0,0 | 4,4 | 8 | kas2_water | 255 (30/70/10 grey) | 8,8 / 0 / 25@4 |
| BF2 YAV1 / NAB2 / DEA1 / FEL1 / DAG1 | 0 | YAV1 DisableLowRes; others far on | 2,2 | .1,.02 / .05 / .001 / 0 / — | 8 / 3 / 6 / 4 | 8/3/3/8 | `<map>_water` | 255 / 128 / 100..200 / 188 | 25@4 (no SpecularEnable → BF2 draws NO speckle, NO far fog) |

**CORRECTED 2026-09-25 (`analysis/re-water-flags.md` §0, `[O]`):** the two "(no SpecularEnable …)" cells above have the
default backwards — unauthored = ON (the level reset stores 1), so BF2's flat water DOES draw the speckle and the far
`fogspread` sub-pass. BF2 FEL1 (and TAT3, which has no WATR) also author `OscillationEnable(0)`, 0x12e39af3: FEL1's water
does not bob (§7 there).

**AMENDMENTS (2026-09-09 evening, queue item W1's report — the port item that consumed this table):**
- The LowRes column above reads "YAV1 DisableLowRes; others far on" for the BF2 row: it is YAV1 **and DAG1** — BF2 DAG1
  authors `DisableLowRes` too, which is W2's far-water rule for that map. (amended 2026-09-09 evening, W1's report)
- `SpeckleTextures`' prefix form expands to **UNPADDED decimal names** (`<prefix>0`, `<prefix>1`, … — not zero-padded),
  which is what W3's speckle pass must load. (amended 2026-09-09 evening, W1's report)

Hashes: Tile 0x713a7cc9, Velocity 0x32741c32, PatchDivisions 0x9036eb, WindSpeed 0x13901a72, WindDirection 0x415e2b30, MainTexture 0x35a83d43, FoamTexture
0x56e7acb7, DisableLowRes 0xa01d9243, LodDecimation 0x807b1583, OceanEnable 0xa69c431a, 0xa015e2ff (→ mode 2, unnamed, unauthored), PhillipsConstant 0xd25ef90a,
FoamTile 0xdcb91b88, LightAzimAndElev 0x9a4f3cc1, Min/Max/BorderDiffuseColor 0xc2a26ee6/0x702173b4/0x44b48fc6, SpecularColor 0x1e4aaf7, SpeckleSpecularColor
0xa6b274b0, SpeckleAmbientColor 0xba5f7683, SpeckleTextures 0x2fa95ff4 (name,count,fps or a list), SpeckleCoordShift 0x6ee80757, SpeckleTile 0xdba69cc0,
SpeckleScrollSpeed 0xe9b91268; BF2 OscillationEnable 0x12e39af3, SpecularEnable 0xd4ddec5f. [V by brute force against the decomp switch]

## 6. Draft `RE_LOG.md` entry
### Water: the PS2 surface, its fog, its level and `CPAT` (2026-09-09, Fable session 17 RE agent; `analysis/re-water.md` when lifted)
Closes HOLD 10(f) and 19(c). BF1 `FUN_00475e08` 538702 (`WATR`) keeps only `LAYR.height` (538743) — the u32, both f32 pairs and the colour word are read and
dropped (538742–538751; BF2 696356–696364), so `LAYR`'s colour has NO byte order to find. `WMAP` bit 3 = "an `LPTC` follows in `CPAT`" (538764–538767; disc: bit-3
cells == `LPTC` count on all 14); a patch REPLACES its cell's flat quad (tile+0x24 either/or, 551350–551359) — `water_glb`'s rule is verified.
**SUPERSEDED 2026-09-17 (§4's CORRECTION, queue WP `7ef36a8`): the draw is a THREE-way, not an either/or, and the port's
old rule was too kind — a patch yields its OWN cell, not the box's 3×3 block. This §6 paragraph is the 2026-09-09 draft
and is kept only as the record of what was written then.** The surface
height is `LAYR.height` + 0.06·sin(1.885 t) (539266, 538346) — `mungefs::water` reads the right field; the YAV1 pool contains the bazaar by authoring. The
water is configured by the LVL's `fx__ Effect("Water")` (keys at 538912–538988, 550347–550408; 27 names cracked, §5 table): `MainTexture`, `Tile`×INFO5,
`Velocity`/INFO5, Min/Max/BorderDiffuseColor (a 16-entry per-vertex LUT on VU1, 551904–551970, alpha 551854–551903 → blend 0x44 at 188/255 on KAS2, 80 on
KAS1, 128 on NAB2), an additive `SpeckleTextures` layer (25 `water_specularmask_NN` frames, on the discs). Fog: the NEAR water is never fogged in either game
(no `fogspread` in `FUN_0048b9c0`'s near branch / B2 708590+); the FAR water (past `NearSceneRange.b`, to 0.9·`FarSceneRange`, only without `DisableLowRes`)
is re-drawn Z-tested no-Z-write with a `fogspread` sub-pass (551405–551448); BF2 gates that sub-pass on `SpecularEnable`, which no map authors → BF2's far
water is unfogged [**wrong default — `SpecularEnable` resets to 1; see §1's 2026-09-25 correction**]. YAV1 authors `DisableLowRes` → no far water at all. KAM1 authors `OceanEnable(1)`: a 16×16 Tessendorf FFT ocean per 160 m cell
(`FUN_00477b98` 539609 Phillips, `FUN_004cea50` 539584 iFFT), base `kam1_water_2` opaque + foam `kam1_water` blended — the port paints the sea with the foam
texture. GS words: near main ALPHA 0x44/TEST 0x50000 (Z-written), far main 0x44/0x51001, speckle 0x48/0x51001, fog 0x144/0x51011, ocean base 0x4100/0x50000.
NOT closed: the VU1 microcode (wave displacement, LUT lookup by the random word), the exact look of the fog pass (a PCSX2 GS dump), `0xa015e2ff` (mode 2).

## 7. Queue items (Opus-safe; every PS2 claim cites `re-water.md` §n = this doc's §n)
W1. [LANDED 2026-09-09 evening — `5758238` + `52e6b54`] **read `Effect("Water")` and bake the PS2's texture and tiling** — `mungefs`: parse the `fx__` chunk's `Effect("Water")`
    DATA/SCOP records (same reader as `sky_`; hashes §5) into a `WaterConfig` {main_texture, foam_texture, tile, foam_tile, velocity, patch_divisions,
    lod_decimation, ocean_enable, disable_low_res, min/max/border_diffuse, specular, speckle {textures, tile, scroll, coord_shift}, light_azim_elev}; manifest
    `ManifestWater` gains it (serde default); `water_glb`: texture = `MainTexture` (fallback: today's rule), metres/repeat = `cell / (Tile·INFO5)` (§3), scroll =
    `Velocity/INFO5`; `LAYR`'s pairs/colour stay recorded, unused (§3); mark the `CPAT` rule and bit 3 verified (§4) and add a test asserting bit-3 cells ==
    patches on the three patch maps. Build: cargo test in `mungefs`, `bake`, `manifest`. GATE: a test reads BF1 KAM1's `MainTexture` = `kam1_water_2`,
    `OceanEnable` = 1, KAS2 `Tile` = (2,2) with INFO5 = 4 off the disc; bake-bytes family count: only the 12 water maps' `water.glb`/manifest move [REBAKE];
    KAM1/KAS2/YAV1 goldens before/after [GOLDENS]. Files: `engine/crates/mungefs/src/{water.rs,lib.rs,+fx.rs}`, `engine/crates/bake/src/water_glb.rs`,
    `engine/crates/manifest/src/lib.rs`.
W2. [LANDED 2026-09-09 night — `9637294` + `5e047c2`] **tint, alpha, Z, and the near/far fog rule in the viewer** — water draw = texture × lerp(MinDiffuse, MaxDiffuse, 0.5)
    (the LUT's mean; a per-vertex hash noise in [Min,Max] is the faithful form, §3) with alpha = mean(Min.a, Max.a)/255 blended `src·a + dst·(1−a)`, depth-tested
    AND depth-written (near record 0x50000, §1); NO fog inside `NearSceneRange.c`, fog only past it; clip the water at `0.9·FarSceneRange` and draw none of it
    past `NearSceneRange.c` on `DisableLowRes` maps (§1); ocean maps (KAM1) draw the base opaque and the foam texture blended by its alpha on top (§3), no waves
    (**the waves LANDED separately 2026-09-17, queue OC `b8ebe59`** — the Tessendorf ocean and the 6 cm bob are stepped every frame).
    Build: cargo test in `game`, `render`. GATE: KAS2 cp11 b90 crop — no white bar above the red line; KAS2/KAS1 goldens show terrain through the water;
    KAM1 vantage sea is `kam1_water_2`-textured and darker than the sky base; YAV1 cp5 unchanged in level [GOLDENS]. Files: `engine/crates/game/src/viewer.rs`,
    `engine/crates/render/src/{lib.rs,shader.wgsl}`.
W3. [TODO — speckle layer, after W2] **the additive animated speckle pass** — a second draw of the water mesh: frame `SpeckleTextures[(t·fps) mod n]` (cross-fade
    two frames), UV = world XZ × SpeckleTile / cell, scrolled by SpeckleScrollSpeed, colour SpeckleSpecularColor, additive blend, no depth write (§3); BF2 maps
    skip it (no `SpecularEnable`, §1). [**Superseded 2026-09-25: BF2 draws it too — `SpecularEnable` defaults to 1
    (`re-water-flags.md`); W3 landed for BF1 and queue WSE adds BF2.**] Build: cargo test in `game`. GATE: a KAS2 golden with the layer toggled shows the ≤ 25-frame specular highlights moving;
    frame-time delta on KAS2 < 5 % [GOLDENS]. Files: `engine/crates/game/src/viewer.rs`, `engine/crates/render/src/lib.rs` (a second material slot).

## 8. Still open / only a PCSX2 GS dump settles
- VU1 microcode 0x515410 (not in the decomp): how the phase nibble animates height/colour, whether the far LOD tiles are displaced, the exact per-vertex alpha.
  **ANSWERED 2026-09-25 for the COLOUR and ALPHA (§3c, `analysis/re-water-colour.md`, `[O]`):** the phase nibble picks
  the vertex's entry of the time-animated alpha table, the index its colour entry; routine B adds `SpecularColor`·F to
  both. The height animation (the bob is EE-side, §2) and the far LOD's displacement are not in this overlay [H].
  **NARROWED 2026-09-17 (§3a, queue OCA): this is the FLAT-water path only.** The ocean uploads neither that program nor the
  LUT block 0x754a30, and its per-vertex alpha is EE-side — the foam byte halved (`FUN_0048a6c8` 550727).
  **NARROWED AGAIN 2026-09-17 (`analysis/re-water-alpha.md`, queue WA's [RE] half, `[O]` UNAUDITED): only the VERTEX half is
  still open.** The TEXTURE half of this hold is answered from the EE side alone — the water's TEX0 has TCC=1 and TFX=0
  (MODULATE) on every layer but the near speckle, whose TCC is cleared on purpose, so `As = At × Av >> 7`. What the microcode
  still owns is Av itself (the phase nibble's animation of the Min.a→Max.a value), not whether the sheet's alpha reaches the blend.
- The far water's fog pass look (fog alpha per vertex is VU-side) and whether the far target's black clear shows below KAM1's dome past 640 m — GS dump / PCSX2 frame.
- Key `0xa015e2ff` (mode 2: draws nothing, no bob; unauthored on every disc), `0xbf2b02c2` and the three PC colour keys — unconsumed by both ELFs.
- The FFT ocean's amplitude (needs running the spectrum; A = 1e-5, L = 63.7 m → metre-scale waves [D]); `FUN_004cea50`'s butterfly not read line by line.
  **CLOSED 2026-09-17 by §3a (queue OCA): the transform is UNNORMALISED** — the `1/(N·M)` divide is gated on a flag the water
  passes as zero, and the mesh carries no metres at all, so the amplitude is the raw transform's own range: **19.23 m
  peak-to-trough / 4.145 m rms** (median of 320 frames). The port's 1.25–1.90 cm (queue OC `b8ebe59`) is that ×1/256 and is a
  defect, fixed by queue **OCB**. The butterfly IS now read line by line (`FUN_004cece8` 593674–593762 = NR `four1`).
- YAV1 bazaar / KAS2 post: the disc's plane is right; whether the port's floor/terrain there is 0.3–1 m low is a port-side measurement (terrain height at (109,−47) vs the CP's −72.22).
  **ANSWERED 2026-10-06 (§9.3, card WADE):** the port's floor is the PS2's there — a free PS2 soldier stands at
  −72.818 on the bazaar floor (the port's `Sim::ground_under` −72.798), 0.32 m under the plane.

# OPUS RE (loop lane 2, 2026-10-06, card WADE) - [O] UNAUDITED: no Fable pass
## 9. Soldiers in water — what the water does to a unit standing in it, and how it is drawn over him

The maintainer's playtest (2026-10-04, BF1 KAS2): "you can't see its body in the water". Tags: [V] read at the line, [M]
measured live on the PS2 (`engine/artifacts/lead_tmp/wade/ps2/`), [D] derived, [H] guess. BF1 lines are
`SWBFRONT.ELF-decomp.c`, BF2 `SWBFRNT2.ELF-decomp.c`.

**The answers.**
1. **Both games draw an opaque model segment BEFORE the near flat water, and the water — blended, Z-tested and
   Z-written — over it.** A wading soldier's legs show THROUGH the water, tinted by the same pass that tints the bed
   (`As = At × Av >> 7`, §3 and `re-water-alpha.md`). The port draws its skinned soldiers AFTER the water, whose depth
   then hides everything under the surface: the maintainer's hard line at the belt. [V][M][O]
2. **A soldier in water walks on the bed at any depth** — no float, no swim, no depth limit, no slow-down. [V][M][O]
3. **Deep water drowns him**: while the surface is at or above his aim origin (feet + 1.80 m standing) a timer runs
   down from 3.0 s; then 30 damage every 1.0 s until he leaves or dies. While the surface is above his weapon's fire
   point (feet + 1.58 m) his triggers are dead. [V][M][O]
4. **The port's sim already stands him where the PS2 does** (22 spots on five maps: 21 within 2.2 cm, one on a
   slope 5.2 cm). The soldier "ON the
   water" at BF2 YAV1's bazaar is the walkabout's own placement, not the game's. [M]

### 9.1 The draw order [V][O]
A draw picks a render BUCKET, and the frame kicks the buckets in ascending index order.

| what | BF1 | BF2 |
| --- | --- | --- |
| bucket selector | `FUN_0049ee68(tex, _, N, _)` 562988: `FUN_0049de40(tex, N, _)` 563013, else `FUN_004a8de8(N)` 563016 = `0x6eeb50 + N*0x14` (567241–567250) | `FUN_00585580` 718365: 718386 / 718389, `FUN_0058a3d8` = `0x7c4d70 + N*0x14` (720969–720977) |
| the kick, ascending | `FUN_004a8f18` 567277: *(decompiler detail omitted)* 567296 … `< 0x15` 567435 | `FUN_0058a598` 721039: *(decompiler detail omitted)* 721054 … `< 0x16` 721172 |
| discarded buckets | `DAT_00522ea8..eb8` (567301–567302): ELF 0xffffffff ×5, no writer | `DAT_0060c510..520` (721058–721059): ELF 0xffffffff ×5, no writer |
| near flat water | `FUN_0048b9c0` 551306 `FUN_0049e2e0` = **bucket 8** (562447); speckle cross-fade 551455 `FUN_0049ebc8` = 8 (562863) | `FUN_00575fd8` 708716 `FUN_00584a70` = **bucket 7** (717831); cross-fade 708901 `FUN_00585348` = 7 (718240) |
| far flat water | 551303 `FUN_0049e240` = bucket 10 (562397) | 708713 `FUN_00584a10` = bucket 9 (717801) |
| near main record | 0x69c390 = {0x4144, TEST 0x50000, 0x58}, `FUN_0048fca0` 553344 / 553360 / 553361: blend `Cs·As + Cd·(1−As)`, Z test, **Z write** (§1 cit. 5) | 0x76c170, the same words, `FUN_0057a3a8` 710717–710745 |
| model segment draw | Normal class `FUN_004cf6f0` 594179: not translucent → `FUN_0049e260` / `FUN_0049ea28` = **bucket 5** (594302, 594310), with draw flag 8 → `FUN_0049e2a0` / `FUN_0049eb58` = bucket 7 (594305, 594313); translucent → 0xd (594319; the 0xc arm 594322 is dead: `DAT_00523010` ELF 0, no writer) | `FUN_0058ed70` 723257, the only segment draw: bucket in *(decompiler detail omitted)*, default **5** (723381); translucent → 0xc (723420) |
| other flags | 0x1000000 → 3 (594345); 0x80000 → 0x10 (594341); 0x2000000 → 0x11 (594332 / 594335); far pass 0x20000 → `FUN_004d0108` = 9 / 10 (594247) | seg 0x1000 → 0xb (723424); 0x80000 → 0xe (723428); 0x2000000 → 3 (723432); far pass 0x20000 → 8 / 9 (723436–723438, 723634); 0x4000000 → 0x12 (723443) |
| soldier's render | `FUN_002013a0` 113090: model slot +0xc with *(decompiler detail omitted)* (114235–114237, 114255–114257) | `FUN_00239bb0` 153024: *(decompiler detail omitted)*, *(decompiler detail omitted)* = 0 or 0x40000000 (155275, 155279–155280) |

- Translucent := colour alpha ≠ 0xff, or the segment's blend mode / flag 4, or a draw flag (B1 594259–594261, B2
  723411–723413). The soldier renders add none of 0x1000000 / 0x80000 / 0x2000000 (B1) or 0x80000 / 0x2000000 /
  0x20000 / 0x4000000 (B2); 0x80001 is passed by the per-viewport model drawn at `hp_camera` (B1 `FUN_001a4250`
  47975, the string at 0x52cb70; B2 `FUN_001c0688` 68393) — the first-person view model [D], which therefore draws
  last (0x10 / 0xe).
- BF1's other three segment classes take the same buckets (callers of the selectors, `re_check.py callers`): Scroll
  `FUN_004b0c48` 5 / 0xd (572578, 572575), Specular `FUN_004b2310` 5 / 0xd (573351, 573354; by the verifier's disc
  census the class of the super battle droid's and the dark trooper's bodies), Blur `FUN_004cf0a8` 0xe (593946). The batch route (594271–594297, `DAT_00523014`
  ELF 1) flushes into bucket 5 (`FUN_004b1a20` 573093 / 573109). Draw flag 8 is set by the model draw itself
  (`FUN_00496248` 557243) for a model with a sub-model list — bucket 7, still before the water.
- **ABSENCE [V], both by the callers and by the verifier's ELF scan of every `jal` to the selectors:** bucket 8 (B1)
  is selected only by the water — `FUN_0048b9c0` and the ripple renderer `FUN_0048d2b8` (552006, 552063, 552130),
  which the dispatcher runs right after the near pass (550911); bucket 7 (B2) only by `FUN_00575fd8` and
  `FUN_00577ad0`. No model path selects the water's bucket.
- **So, per near pass:** terrain (3) → opaque models, soldiers among them (5, BF1 also 7) → the near water (8 / 7)
  → the far scene → translucent model segments (0xd / 0xc) → the first-person model. [D from the rows above]
- **What follows for a translucent segment** [D]: it is drawn after a water that wrote Z, so under the surface it
  is not drawn. A soldier fading out near the near / far boundary (the model draw's distance fade, B1 557246–557256,
  B2 712818–712829) is such a draw. BF2's 0x1000 segments (bucket 0xb) are 35 segments of buildings, no soldier
  (the verifier's disc census).
- **Scope:** the FLAT water only. The ocean (KAM1) puts its near base in bucket 5 with no blend (B1 `FUN_0048b280`
  551127 `FUN_0049ea28`; B2 `FUN_00575670` 708515 `FUN_00585208` = `FUN_00584cb8(5, …)` 718190) — opaque, and
  ordered against the models by texture page, not by bucket. The order holds per pass: the far pass draws its
  opaque segments (9 / 8) after the NEAR water.

**Live, the PS2's own frames** (`engine/artifacts/pcsx2/ref/<map>/wade/`, each a free-standing soldier; `<spot>.png`
water on, `<spot>_nowater.png` the same stand with the water's draw switched off by one live data write — BF1
`DAT_00522a78` = 1, BF2 the byte `DAT_0060c1f8` = 1, both ELF 0 with no writer, read by the dispatchers at 550906 /
708256): BF1 KAS2 at 0.96 m and 1.50 m, BF2 FEL1 at 0.61 m and 1.18 m, BF2 YAV1's bazaar at 0.32 m. In every
water-on frame the submerged body is there, tinted; in the water-off frame it is the plain soldier on the dry bed.
The legs keep 0.54 / 0.82 / 0.83 of their contrast against the bed on the same screen row at the three spots with
more than 3,600 submerged pixels (`tools/wade_check.py`; rough — his idle pose moves between the two frames), where
`1 − As` predicts 0.73 for KAS2 (At 48, Av 94 → 35/128) and 0.51–0.77 for FEL1 (At 40–86). [M]; the equality with
the bed's blend is [D] from the draw order, not measured to better than that.

### 9.2 The soldier update's water block [V][O]
BF1 `FUN_00205cc8` (116426) 117186–117270; BF2 `FUN_00240bf8` (158414) 159994–160069, the same block statement for
statement (the six float immediates sit in the same order in both ELFs: 30.0 at 0x206efc / 0x242e8c, 0.3 at 0x206f7c
/ 0x242f70, 4.0 at 0x206fa4 / 0x242f98, 2.0 at 0x206fe8 / 0x242fd8, 0.25 at 0x206ff4 / 0x242fe4, −5.0 at 0x20704c /
0x243048).

| step | BF1 | BF2 |
| --- | --- | --- |
| water height at his XZ | `FUN_004768f0` 117186 → `DAT_00564908 + DAT_005229cc` (LAYR + bob) in a wet cell (539032) | `FUN_005613c8` 159995 → `DAT_00661604 + DAT_0060bef0` (696617) |
| dry, or water ≤ feet y | timer soldier+0xc28 ← class+0x1e54 (117191, 117195) | timer E+0xac4 ← class+0x1218 (159999, 160002–160005) |
| water > soldier+0xad4 / E+0x604 (the fire point's y) | the two trigger byte pairs soldier+0x2e8..0x2eb zeroed (117200–117208) | both words E+0x2a8 / E+0x2ac `& ~0x1f \| 0xc` (160007–160014) |
| alive and water ≥ soldier+0x3a4 / E+0x344 (the aim origin's y) | timer −= dt (117220–117221); < 0: += 1.0 (117232), 30.0 damage (117233) → `FUN_001bc8e0(health, rec, −1)` 117240 | 160016–160023; 30.0 at 160027 → `FUN_001e0900(E+0x170, rec, −1)` 160040 |
| else | timer ← class (117213) | 160017 |
| ripple | global `DAT_0051789c += dt`; period `1 / (0.3·\|v\|² + 4)` s (117251, v 3-D); `FUN_00478778(2.0, 0.25, pos, v)` 117254 + foley surface water 117256 | `DAT_005fd0a0`, 160052, `FUN_00563230` 160055, 160057 |
| splash | `v.y < −5` and ≥ 1 s since the last (global `DAT_005178a0`): effect class+0x1900 at pos (117263–117268) | `DAT_005fd0a4`, effect class+0x8c4 (160064–160069) |

- The timer's reload is **3.0** at the soldier class constructor (B1 `FUN_0020d238` 121979, `swc1 f22,0x1e54(s2)` at
  0x20d82c with f22 = `lui 0x4040`; B2 `FUN_00250488` 169616, 0x250ba4) and is copied to a child class (122143 /
  169942). **ABSENCE:** no property setter writes it — every other hit of the offset is a read or another class
  (B2's 125522 / 125658 are the FLYER class's setter `FUN_0020fd58`); the verifier's ELF scan found three stores per
  game, constructor, copy and the other structure. It is not an ODF value. (The verifier's find, not re-derived
  here: a ninth `DropItemProbability` entry would overrun onto it — B1 122961, B2 171670 — and by its disc census no
  class authors more than seven.)
- The aim origin soldier+0x3a0 is written by `FUN_00204748` (115523–115586, 115678–115681), `FUN_001ff1e8`
  (111735–111738) and the reset `FUN_0032f820` (317729–317732), which builds it from the class's per-STANCE section
  (class+0x1920 + stance·0x60, 317727) — so a crouched soldier's is lower [D; not measured].
- **ABSENCE:** the block writes no position, velocity or speed factor (B1's only stores in 0x206df0–0x2070b8 are the
  trigger bytes, the timer, stack slots and the two globals — the verifier's ELF list; *(decompiler detail omitted)* is on both
  sides of the wet test, 117190 / 117199). The ripple and splash timers are GLOBALS, one for all soldiers.
- The ripple record (`FUN_00478778` 540079): flat-water maps only (`DAT_005647b8 == 0`), at most 100 (540103), in a
  wet cell (540118–540120), **only when `|v|² > 0`** (540127); at (x + 0.03·vx, LAYR, z + 0.03·vz) (540178–540180),
  life 2.0 s, size 0.25. Drawn by `FUN_0048d2b8` in the water's bucket (radius = size + 1.5·age at 552055; the
  rest of that renderer is unread [H]).

**Live, one row per game update** [M] (`wade_probe.py`: the soldier held 1.2 m over a spot for 0.5 s — BF1 C+0x228
and C+0x198, BF2 E+0x150 — then released; no health pin):

| | BF1 (stormtrooper, 300 hp) | BF2 (stormtrooper, 400 hp) |
| --- | --- | --- |
| feet on the bed, stance 0 | KAS2 depth 0.41 / 0.59 / 0.96 / 1.31 / 1.50 / 1.85 / 2.93 m, KAS1 7.28 m | FEL1 0.27 / 0.61 / 1.18, YAV1 0.32 / 0.40 / 0.72 / 1.17 / 1.31, NAB2 2.39 / 3.46 m |
| aim origin, fire point over the feet | +1.800, +1.583 | +1.800, +1.583 |
| class reload read from RAM | 3.0 | 3.0 |
| triggers with R1 held | read 0 at 1.85, 2.93, 7.28 m; live at ≤ 1.50 m | `…0c / …0c` (masked) at 2.39 and 3.46 m; press edge `…03` at ≤ 1.31 m |
| drowning | KAS1: under at ≈ 0.57 s, −30 at 3.570 s, then every 1.001 s, dead on the 10th tick (12.58 s) | NAB2: −30 at 3.604 s, then every 1.001 s, dead on the 14th tick (16.58 s) |
| walk speed in water | 6.47–6.50 m/s at 1.0–1.4 m (MaxSpeed 6.5) | 6.35–6.75 on sloping beds (MaxSpeed 7.0; no flat control) |

- At exactly 1.85 m (KAS2) the 6 cm bob lifts the surface over the aim origin for 2.7 s of every 3.33 s: the timer
  reloads each time it dips and never fires in 16 s. FEL1 authors `OscillationEnable(0)`: the bob word read 0.
- One BF1 death is NOT the water's: at KAS2 (384, 256) the soldier went from 120 to 0 in one update with the timer at
  0.06 s, 9.5 s after the teleport — an instant kill [H: the battlefield boundary; BF2's frame at FEL1 (−121.7,
  −118.6), where he also died within seconds, shows "Leaving Battlefield":
  `pcsx2/ref/bf2_fel1/wade/leaving_battlefield_s105.png`]. KAS1's run is the clean one.
- A cell that is not wet has no water for him: at that FEL1 spot, dry ground 4.78 m under the plane's height, the
  timer stayed 3.0 for the 5 s he lived [M]; the wet test is the query's own (`FUN_00477458` / `FUN_00561ef0`).
- **The stand, PS2 against the port** (`tools/wade_ground.rs`, `Sim::ground_under` at the PS2 soldier's XZ): 22
  spots — KAS2 7, KAS1 1, FEL1 3, YAV1 9, NAB2 2. 21 agree within 2.2 cm (the PS2's feet lower at 19 of them, by
  up to 2.2 cm; higher at two, by up to 1.5 cm); on NAB2's bank (−60.45, −64.79), a slope he slid down, the PS2 is
  5.2 cm lower. BF2 YAV1's bazaar: the floor is −72.818 on
  the PS2 at four spots 3 m from the post (the port −72.798) and two more 6 m out, 0.32 m under the plane; AT the
  post he stands on the post's own base, −72.234 (the port −72.214), 0.27 m above it.

### 9.3 What the port owes
- **Render (card WADE, build):** the skinned opaque draws and their specular passes go before the water runs in
  `render/src/frame.rs` (they are the last depth-writing draws today, after `DrawLayer::Water`); skinned blend draws
  stay in the blend pass, where the water's depth hides them as bucket 0xd / 0xc are hidden. Referee:
  `lead_tmp/wade/tools/wade_check.py` replays the five PS2 stands and poses — main 2560be8: 6–22 % of the submerged
  silhouette shows at the four deeper spots with a transmission of 0.05–0.18 (the fringe above the estimated
  waterline), 73 % and 0.82 at the bazaar's 0.32 m; a pass wants ≥ 90 % and 0.3–0.95 at all five.
- **The walkabout's reference soldier** stands at `max(terrain, the post's y)` (`gallery::ground_stand`): 0.59 m
  over the bazaar floor. It should stand on the floor the sim gives.
- **Sim (card WDRN, implemented 2026-11-01):** §9.5 records the drowning and trigger rules. The crouched and prone aim origins remain provisional pending measurement.
- **Effects (note on card FXP5):** the ripple ring behind a moving wader and the splash on a fall into water.

### 9.4 Checks run
- `re_check.py fn` on every cited line (both games; pasted in `lead_tmp/wade/claims.md`'s source run), `callers` on
  every bucket selector, `absent` on 0x1e54 / 0x1218, `word` + `writers` on the two discard sets, the two water
  switches, `DAT_00523010` / `DAT_00523014`; `lead_tmp/wade/tools/lui_scan.py` for the float immediates.
- Twins opened at every hop: selector, kick, flat renderer, segment draw, soldier render, soldier update, class
  constructor. BF2's flat renderer is reached from the dispatcher `FUN_005755b0` (708259), as BF1's.
- One Opus verifier, claims and citations only (`lead_tmp/wade/claims.md`, 24 claims): **22 SUPPORTED, 2 PARTLY, 0
  NOT.** PARTLY and folded in: the water query's store is line 539032, not 539029; the aim origin has a third writer
  (`FUN_0032f820`). It added the ocean's bucket 5, the far pass's order, the dead 0xc arm, BF1's other segment
  classes and the model draw's own flag 8, each re-opened here before it was written above.
- **Stayed [H] / open:** how a bot treats deep water (nothing in this block is AI); the ripple renderer past its
  radius line; why the KAS2 (384, 256) soldier died; the crouched and prone aim origins; whether a first-person
  view under water changes anything (not looked at).

### 9.5 Soldier water rules in the port (2026-11-01, issue #70)

The existing WMAP mask now shares a fixed-tick layer-height query with the PS2's
6 cm, 1.8849558 rad/s bob. Both ocean modes suppress the bob; BF2 also honors
`OscillationEnable(0)`. The phase starts at the port's round tick zero and has not
been aligned to a PS2 capture. The same bobbed plane now stops bolts, grenades and
missiles and sets a droideka's depth, as the PS2's one query and crossing do (`re-fx.md`
§15). The bots' hold-spot and ring-point checks are the port's own rule and stay at
the layer height, so a spot does not come and go with the bob. Older manifests without WMAP retain the existing
whole-plane fallback; current water-map bakes carry the mask.

A living soldier on foot takes 30 damage at 90 submerged 30 Hz updates, then
every 30 updates. Surfacing below the stance eye or leaving a wet cell restores
the 90-update breath timer. Integer ticks preserve exact three/one-second waits
instead of adding a tick through float subtraction. Standing eye height is
1.80 m; crouch/prone use the existing 1.30/0.50 m camera heights provisionally.
Water damage uses the damage record's unit multipliers and shield scale, respects
the invincibility cheat, and deaths carry the existing environment-water cause.
Mounted riders and droidekas retain their separate rules; droidekas already have
a class-authored water-damage path. Movement and bot path selection are unchanged.

The fire-point comparison uses the existing stance/pitch transform: at standing
spawn pitch −0.07, its height is 1.583 m. Both triggers are masked and queued
salvos are cancelled while submerged. This also prevents emitter shots, whose
cone query is reached only after a weapon cycle produces a shot. Previously
launched projectiles retain their own collision and life rules.

`core/tests/drowning.rs` replays the measured death intervals, shallow/dry
controls, surfacing and bob resets, both triggers, queued salvos, stance/pitch
heights, unchanged wading movement, invincibility and deterministic breath state.
No fresh PCSX2 capture or deep-water AI avoidance rule is claimed.
