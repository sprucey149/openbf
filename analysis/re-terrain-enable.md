# OPUS RE (loop lane 2, 2026-09-28) - [O] UNAUDITED: no Fable pass
# RE: `TerrainEnable` — BF1 Kamino and Bespin Platforms switch their terrain off; the PS2's Kamino sea is the ocean alone (card KOC)

**The question** (card KOC; `engine/artifacts/pcsx2/ref/FINDINGS.md` F9; `analysis/re-water-alpha.md` §5's open [H]).
BF1 Kamino's sea on the PS2 is dark teal under white foam crests. In the port it is a flat, textureless, pale plate: the
map's y = 0 heightfield, textured with `kam1_water` (LTEX[0]) and drawn opaque, 8 m above the ocean at y = −8. The card
asked whether the terrain pass takes that sheet's alpha, and why the plate shows no texture.

**The answer is one level up: the PS2 never draws this terrain.** The map's sky switches it off.

**Method.** The loop's RE discipline: the hops below, the five self-checks pasted verbatim in
`engine/artifacts/lead_tmp/re/koc/checks/` (`fn.txt`, `absent.txt`, `word.txt`, `word_asm.txt`, `writers.txt`,
`callers.txt`, `scan_*.txt`), then one foreground Opus verifier given the claim list only (`koc/claims.md` →
`koc/verify/VERDICTS.md`, §10). The PS2 was taken apart with patched save states (§4). The port was rendered at the same
poses (§7).

**Scratch** (`engine/artifacts/lead_tmp/re/koc/`): `skykeys.py` (every `sky_` key on both discs), `hashscan.py` (a
32-bit constant in an ELF), `tex.py` (the two sea sheets), `ps2exp/` (`capture.py`, `mkstates.py`, `run_ps2exp.py`,
`means.py`, the frames and sheets, and the F9-pose state `states/koc_sea23.p2s`), `port/` (the port renders,
`means_port_vs_ps2.txt`, and a README with the scratch renderer's one-block change).

**Conventions.** B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`. ELF file offset =
va − 0x160000 + 0x1000. Key hashes are fnv1a-32 over the lower-cased name.

## 0. The answer

| | BF1 | BF2 |
|---|---|---|
| **the key** | `DomeInfo { TerrainEnable(n) }` → `DAT_005229a8` = (n ≠ 0); reset to 1 when the scope opens | not read: the hash is in neither the decomp nor the ELF |
| **authored** | KAM1 and BES1, both `TerrainEnable(0)` | nowhere |
| **at 0** | no terrain draw of any kind (patches, detail pass, far sheet); the height query answers −1000 and the terrain ray never hits | — |

- **The PS2's Kamino sea is the ocean draw alone, over black.** Measured on the PS2 at the F9 pose (§4): switching the
  terrain draw off changes nothing, and switching the water off turns the whole sea black.
- **The live PS2 RAM agrees with the disc:** the flag is 0 on BF1 KAM1 and BES1 and 1 on the other 13 BF1 maps (§3).
- **Why the port's plate is textureless:** `kam1_water` is flat grey (180,180,180) on every texel. Its foam is all in
  the alpha channel (0..218), so an opaque draw of it can only be a flat plate (§6).
- **The port reads no `TerrainEnable`.** It draws KAM1's heightfield. On BES1 it drops the draw only because the layer
  texture is missing from BES1.LVL. On both maps the sim stands soldiers on that heightfield (§7).
- **Without the heightfield, the port's own ocean is close.** In F9's `v4_down` box the port reads (20,33,29) against
  the PS2's (17,30,33); today it reads (191,214,237). What remains is the ocean's foam contrast (per-channel spread 6
  against 16), card FOAMT.
- **re-water-alpha §5's [H] is answered and moot.** Were a terrain pass to run with this sheet, it would composite by the
  texture alpha (As = At, §6.2). Neither map that names the sheet runs a terrain pass.

## 1. The key [V][O]

1. **The scope.** `FUN_002e5148` hands the sky scope with name hash 0x185ab6c2 ("DomeInfo") to `FUN_002e29e0` (B1
   271557, 271581). `FUN_002e29e0` also parses its `LowResTerrain` sub-scope (0xddf0c29e, 270346).
2. **The reset.** On entry `FUN_002e29e0` stores `DAT_005229a8 = 1` (270060). ELF: 0x2e29ec `addiu a2,zero,1`,
   0x2e29fc `lui a1,0x52`, 0x2e2a50 `sw a2,0x29a8(a1)`.
3. **The key.** For hash 0xae291b6d ("TerrainEnable") it stores `DAT_005229a8 = (FUN_003947c8(value) != 0)`
   (270354–270356). ELF: 0x2e2c24 `lui v0,0xae29`, 0x2e2c28 `ori v0,v0,0x1b6d`, 0x2e2c2c `beq v1,v0,0x2e2d28`;
   0x2e2d28 `jal 0x3947c8`, 0x2e2d30 `sltu v0,zero,v0`, 0x2e2d34 `lui a0,0x52`, 0x2e2d3c `sw v0,0x29a8(a0)`.
   - `FUN_003947c8` converts to unsigned, so a negative value would read 0 (verifier). No disc authors one.
4. **Default and writers.** The ELF word at 0x5229a8 is 1. Those two stores are its only writers:
   - 2 decomp writers, both in `FUN_002e29e0`;
   - an lui-tracked scan of the whole executable finds exactly 2 stores (0x2e2a50, 0x2e2d3c), and so did the
     verifier's own scan;
   - the two pointers built within 0x20 below it (0x46f178 → 0x522988, 0x46f2d0 → 0x522998) feed `lh` table loads.
5. **BF2 dropped the key.** Neither the BF2 decomp nor the BF2 ELF holds 0xae291b6d (no lui+ori/addiu pair, no data word).
   The same scan finds the sibling key 0x6ef738fb in both ELFs (BF1 0x2e3184, BF2 0x3bd234).

## 2. What the flag gates [V][O]

1. **The terrain draw.** `FUN_00487ad8` is the TerrainRenderable's render method:
   - vtable 0x558da8, whose slot 1 `FUN_00488e58` registers the RTTI name "17TerrainRenderable" (0x558df8, 549933) and
     whose slot 6 (0x558dc0) is 0x00487ad8; the constructor `FUN_004887b8` stores it.
   - Its first statement, `if (DAT_005229a8 != 0)` (549130), encloses the whole body: the near patch loop (`FUN_004857b8`
     549275, `FUN_00485928` 549279), the DTLX detail pass `FUN_00486e30` (549301), `FUN_00487908` (549315) and the far
     branch `FUN_00488268` (549319).
2. **The far-scene call.** `FUN_002f2308` calls the terrain only under the same test:
   *(decompiler detail omitted)* (280296–280297), where `FUN_00481300` is
   *(decompiler detail omitted)* (545762).
3. **Gameplay.**
   - The height query `FUN_00473710` (34 call sites) takes −1000.0 for every sampled corner when the flag is 0 (537066,
     537073, 537080 and nine more).
   - The terrain segment test `FUN_00474680` returns 1.0 (no hit) without testing (537629–537631).
   - `FUN_00474810` tests the terrain only when the flag is set (537695).
   - Twelve more functions test the flag in the decomp text (`checks/absent.txt`), and `FUN_00381408` reads it
     through an alias the text search misses (`*(0x520000 + 0x29a8)`, 372192; verifier). They are unread here — the
     card TENS starting point.
   - All 28 calls of the height sampler `FUN_00382338` sit behind the flag: 12 in `FUN_00473710`, 12 in
     `FUN_00473e08`, 3 in `FUN_00487020`, 1 in `FUN_00381408` (verifier, reproduced).
4. **So on KAM1 and BES1 the PS2 has no terrain at all:** nothing drawn, and nothing to stand on or hit.
5. **Side finding for TDTL (verifier, reproduced).**
   - `FUN_00487908`'s gate `DAT_005229b4` is 0 in the ELF and has no store in the decomp or the ELF: dead on retail.
   - The detail pass's gate `DAT_005229b0` is 1 in the ELF. Its only stores are two tiny functions missing from the
     decomp (0x472690 toggles it, 0x4726a8 sets it), and nothing jumps to them or builds their address: it stays 1.

## 3. The disc and the PS2's RAM [V measured]

- **The disc** (`skykeys.py`, `checks/absent.txt`). Of the 51 LVL files on both discs that carry a `sky_` chunk, exactly
  two author `TerrainEnable`: BF1 `BES/BES1.LVL` and `KAM/KAM1.LVL`, each `DomeInfo { TerrainEnable(0) }`. The verifier's
  raw search of all 770 disc files agrees.
- **The RAM.** The kept in-level BF1 save states (`engine/artifacts/pcsx2/ref/states/`) hold 0 at 0x5229a8 on BES1 and
  KAM1. They hold 1 on BES2, END1, GEO1, HOT1, KAS1, KAS2, NAB1, NAB2, RHN1, RHN2, TAT2 (Mos Eisley), YAV1 and YAV2.
- **No far sheet either.** BF1 KAM1's `tern` has no `LOWR` chunk. `FUN_00488268`'s cell count (+100) is set only from a
  `LOWR`'s INFO (546096–546104 in `FUN_00481608`), so the far branch would draw nothing even with the flag set.

## 4. The PS2 taken apart [V measured]

**The instrument.** `ps2exp/capture.py`, one scripted PCSX2 run under `ci.lock`:
- it boots the kept BETA1 spawn state, turns and walks the soldier to the library's `v4_down` spot (reading the camera
  from RAM between key holds), pitches down, and quick-saves;
- `koc_sea23`: eye (−1.12, 64.18, 422.19), yaw 188.6°, pitch −23.2° — F9's view;
- `mkstates.py` patches one word per copy and checks every word first; `run_ps2exp.py` reloads each copy and saves its frame.

| variant (`koc_sea23`) | patch | box (250,40,630,200) RGB |
|---|---|---|
| ctl, ctl2 | none | (12,23,25), (12,23,25) |
| noterr | TerrainRenderable slot 6 (0x558dc0) → `FUN_0049e028` (`jr ra; nop`) | (13,23,25) |
| c400 | `DAT_00677664` (NearSceneRange.c, re-read every frame, 540734 / 540763) 90 → 400 | (13,23,25) |
| **nowater** | `DAT_00522a78` = 1 | **(2,4,8): the sea turns black** |
| terron | `DAT_005229a8` forced to 1 | (5,11,15) |
| terron_c400 | + c = 400 | a dim smooth band over the sea |
| terron_c400nw | + no water | (12,15,21): a dim band over black |

- **The water gate.** `DAT_00522a78` is read once, as `FUN_0048b1e0`'s first test (550906; ELF load 0x48b1e8), and never
  written. Its ELF value is 0. `FUN_0048b1e0` is the only caller of the ocean renderer `FUN_0048b280` (550915).
- **So the PS2's Kamino sea is the ocean draw alone, over black.** noterr and c400 equal the controls because the flag
  is already 0.
- **Forcing the terrain on** darkens and hazes the sea. At no setting does the PS2 draw a bright opaque plate.
  - The band's shape (a fade with distance) is [H]: the near scene's depth composite, unread here.
- Sheets: `ps2exp/sheet_koc_sea23_ctl-ctl2-noterr-nowater.png`,
  `ps2exp/sheet_koc_sea23_c400-terron-terron_c400-terron_c400nw.png`. Numbers: `ps2exp/means_sea23.txt`.

## 5. The ocean on BF1 KAM1 [V][O]

1. **A far-scene draw only.** The near WaterRenderable (object 0x6774f0; vtable 0x558720 stored at 540217; RTTI
   "15WaterRenderable") is registered only when OceanEnable (`DAT_005647b8`) is 0 and a water exists (`DAT_005648dc`).
   - The tests are B1 538827–538828 (`FUN_00475e08`) and 538876–538877 (`FUN_004763d0`).
   - They run at the end of whichever of the two parsers finishes second (`DAT_0067752c == 2`), after its key loop.
   - `DAT_005647b8`'s other writer (key 0xa015e2ff → 2 or 0) is authored on no disc file (verifier).
2. **The far call.** The far-scene tick calls the water with 0x20000 (280299) → `FUN_0048b280(0)` (550915).
   - The param-0 branch builds the frustum [`DAT_00677660` = NearSceneRange.b, 0.8 × `DAT_00677668` = FarSceneRange]
     (551035–551039; ELF 0x48b624 `lui at,0x3f4c` / 0x48b628 `ori at,at,0xcccd` = 0.8).
   - It draws the base, foam and `fogspread` records (0x69c3d0 / 0x69c3e0 / {0x144, 0x50010, 0x50}, 551072–551097).
   - KAM1 authors `NearSceneRange(0.8, 60, 90)` and `FarSceneRange(800)`, so the ocean spans 60–640 m.
3. **[D] What a falling soldier would see.** Nearer than 60 m the far frustum clips the ocean, and no terrain covers it,
   so the sea is black under a soldier who falls within 60 m of it. The port need not copy this: it is a hardware-era
   range, not a look.

## 6. The sheets, and the terrain pass's alpha

1. **The sheets** [V measured, `tex.py`].
   - `kam1_water` (256×256, 8 bpp) is RGB (180,180,180) on every texel (std 0), with alpha 0..218 (mean 62.2).
   - `kam1_water_2`, the ocean base, is RGB mean (20.3, 32.7, 28.5) with alpha 255.
   - So `kam1_water` is a foam MASK. Drawn opaque, as the port draws its terrain, it is a flat plate. That is F9's
     "spread 1.0", now brighter since TERL's COLR scale.
2. **The terrain pass would take the sheet's alpha** [V code][O]. This answers re-water-alpha §5's [H].
   - The bind: `FUN_004857b8` binds each layer by `FUN_0049e1e0(DAT_00754638[k], 0, 0)` (548047). That is
     `FUN_0049ee68(tex, prev, 3, hash)` (562367) → `FUN_0049de40` (563013) → `FUN_0049c5c8` (562295), which sets
     TCC = 1 for every non-32-bpp sheet (562034).
   - The pass: descriptor 0x69c2e0 = {0x100044, 0, 0x50000, 0x58} (549875) gives ALPHA 0x44, no alpha test, and ABE in
     the prim flags.
   - The vertex alpha: a one-layer patch's is 0xee → 0x80 (547912), so As = At.
   - It never matters on the disc: both maps that name the sheet switch the terrain off.
   - [H] as in re-water-alpha §3: the VU1 microcode's handling of TEX0.

## 7. The port [V measured]

- **It reads no `TerrainEnable`.** `mungefs::sky::parse_dome` drops the key into `Sky::unknown`, and nothing under
  `engine/crates/` names it.
  - `game::viewer::scene::upload_scene` draws KAM1's heightfield with `kam1_water`.
  - BES1's is dropped only because `kam1_water` is not in BES1.LVL (`terrain_dropped`; the comment there).
  - `Scene::heightfield` gives both maps a ground.
- **The port at the four F9 poses and `koc_sea23`** (`port/`; a scratch copy of `pxc_pose` whose `PXC_DROP_TERRAIN=1`
  drops every `LightPath::Terrain` and `TerrainLayer` draw — `port/README.md`). Columns are box mean, per-channel spread,
  high-pass energy:

| pose, box | PS2 | port today | port, no heightfield |
|---|---|---|---|
| `v4_down` (250,40,630,200) | (17,30,33) sd 16.2 hp 3.3 | (191,214,237) sd 2.5 hp 0.0 | (20,33,29) sd 6.0 hp 3.0 |
| `v5_down` (330,60,630,200) | (20,28,29) sd 12.5 | (121,134,147) sd 94.9 | (22,29,27) sd 9.9 |
| `v2_water` (470,250,640,330) | (14,24,24) sd 6.5 | (187,209,232) sd 7.2 | (19,31,27) sd 4.4 |
| `v3_water` (330,240,560,330) | (41,53,54) sd 33.7 (a lightning bolt) | (185,208,230) | (19,31,27) sd 4.6 |
| `koc_sea23` (330,60,630,130) | (13,23,24) sd 12.2 | (192,215,238) sd 0.8 | (20,33,29) sd 5.4 |

- **Without the heightfield** the port is within 2–7 levels of the PS2's mean on every clean box. Its foam is fainter
  and finer (spread 4–10 against 6–16), with a visible regular tiling: the foam layer's `FoamTile`, card FOAMT.
- Sheet: `port/sheet_port_vs_ps2.png`. Numbers: `port/means_port_vs_ps2.txt`.

## 8. What the port owes

- **KOC (build, M1).**
  - `mungefs::sky` parses `DomeInfo.TerrainEnable`, and the bake carries it into `ManifestSky` for BF1 maps only (BF2's
    ELF never reads it).
  - When it is 0, `upload_scene` draws none of the heightfield: the base, the TALPH layers, and TDTL's detail pass once
    it exists.
  - Moves: `goldens/maps/bf1_kam1.png`, the sea going from the plate to the ocean (and the walkabout statistics with it);
    `rebake = maps`. BES1's picture does not move: its terrain is already not drawn.
  - Pairs to beat: F9's four (`engine/artifacts/pcsx2/ref/bf1_kam1/pairs/cp0_cp2_beta1_{v2_water,v3_water,v4_down,v5_down}.png`),
    with the §7 numbers.
- **TENS (new, M3).** The PS2 gives these two maps no terrain floor or ray hits (§2.3); the port's sim stands soldiers on
  the heightfield. What kills a soldier who falls on the PS2 is still to read:
  - KAM1 has a death region below the sea (the port's manifest: a cylinder at y ≈ −256);
  - BES1 has none in the port's manifest; `FUN_00473710` answers −1000 there.
  - Moves the sweep on both maps.
- **FOAMT (existing).** Once KOC lands, BF1 KAM1's ocean is on screen, and its foam contrast is the remaining gap (§7).

## 9. What stays [H]

- The near-scene depth composite: what the forced-on terrain's fading band is made of (§4). It does not bear on the port.
- The twelve other gameplay readers of `DAT_005229a8` (§2.3) → TENS.
- The VU1 microcode's handling of TEX0 (§6.2), as in re-water-alpha §3.

## 10. Verification

- **Self-checks** (`koc/checks/`, verbatim):
  - **fn:** every cited line is in the named function (two rounds, the second after correcting guessed line numbers).
  - **absent:** 0xae291b6d is at 1 site in B1 and 0 in B2, in the ELF scans of both, and authored in 2 of 51 sky files.
    `DAT_00522a78` has 1 reader. KAM1's `tern` has no `LOWR`.
  - **word:** the flag's initial word 1; the water gate's 0; the TerrainRenderable and WaterRenderable vtables and RTTI
    names; `FUN_0049e028`; the compare and both stores disassembled; the ocean's 0.8.
  - **writers:** `DAT_005229a8` has 2 writers (decomp and ELF scan); `DAT_00522a78` has none; the range globals
    `DAT_00677660/64/68` and `DAT_005647b8` are listed.
  - **callers:** every function in the chain.
- **The verifier** (one Opus agent, claim list only; `koc/verify/VERDICTS.md`, 182 tool uses): **26 SUPPORTED, 0 PARTLY,
  0 NOT SUPPORTED.** Folded in: the `fogspread` record in §5.2; "34 callers" read as 34 call sites; the unsigned
  conversion; the `DAT_005648dc` condition and OceanEnable's second writer; the TDTL side finding in §2.5, reproduced
  here.
