# OPUS RE (loop lead, 2026-09-17, lane 1) - [O] UNAUDITED: no Fable pass

**Does the PS2's water layer composite by its TEXTURE alpha?** — queue item **WA**'s `[RE]` half, i.e. the
first clause of `analysis/re-water.md` §8 / HANDOFF HOLD 21. Method: `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md`
§5 — the four self-checks, then ONE Opus verifier agent given the citations alone (its one NOT SUPPORTED
verdict is §7, reproduced by the lead before it was accepted). Scratch, census binaries and raw measurements:
`engine/artifacts/lead_tmp/re/water-alpha/`. Corpora: `analysis/SWBFRONT.ELF-decomp.c` (BF1),
`analysis/SWBFRNT2.ELF-decomp.c` (BF2); ELF words by `scripts/tools/elfrd.py`, instructions by this
item's `lead_tmp/re/water-alpha/mipsdis.py`. Tags: [V] read at the line / measured, [D] derived, [H] guess;
every load-bearing claim also carries [O] — Opus-only, unaudited.

## 0. Answer

**YES, on both discs, for every BLENDED water layer except the near speckle.** [V][O] Those draws take As
from the fragment alpha after the texture function, and the texture function is MODULATE on a TCC=RGBA
texture, so

    As = At x Av >> 7        At = GS texture alpha = (disc alpha x 0x81) >> 8;  Av = the per-vertex alpha

| water layer | record (BF1 / BF2) | ALPHA | TEX0 TCC | what decides As |
|---|---|---|---|---|
| near main | 0x69c390 / 0x76c170 | 0x44 source-over, Z-written | **1** | texture x vertex |
| far main | 0x69c3a0 / 0x76c180 | 0x44, no Z write | **1** | texture x vertex |
| far speckle | 0x69c3b0 / 0x76c190 | 0x48 additive | **1** | texture x vertex |
| **near speckle** | 0x69c3b0 / 0x76c190 | 0x48 additive | **0 — cleared on purpose (§7.1)** | vertex alone |
| ocean BASE | 0x69c3c0+0x69c3d0 / 0x76c1a0+0x76c1b0 | 0x4100 = no blend, prim 0x18 (no ABE) | 1 | nothing: opaque |
| ocean FOAM | 0x69c3e0 / 0x76c1c0 | 0x44 | **1** | texture x vertex |

The disc's own authored data agrees with its own blend: the ONLY water sheet on either disc that carries
alpha 255 on every texel is `kam1_water_2`, the ocean BASE — the one layer that is not blended (§4).

**For the port:** `render/src/shader.wgsl`'s `fs_water` multiply of `base.a` IS the disc's arithmetic (the
port's 0..255 decode divided by 255 equals the GS's At/0x80 after the `pal_` loader's `(A*0x81)>>8` scale,
`re-hud-icons.md` §1 citation 3). It is inert only because `bake::alpha` forces every map water sheet opaque.
Giving those sheets their disc alpha back — queue **WA**'s build half — is what the disc says, not a look
judgement.
**Corrected 2026-09-23 (cleanse):** WA's build half LANDED 2026-09-23 (`19975d0`) — the named water
sheets keep their disc alpha in the shipped bake (`bake::alpha`'s `water_named` clause).

## 1. The hops, BF1, each with its enclosing function (self-check 1)

1. **The flat-water renderer is `FUN_0048b9c0`** (decomp header 551182). Its driver `FUN_0048b1e0` (header
   550901) calls it at 550909 when `DAT_005647b8 == 0` and the OCEAN renderer `FUN_0048b280` at 550915
   otherwise. `FUN_0048b9c0` binds the water's `MainTexture` (`DAT_0056494c`) at **551303**
   (`FUN_0049e240`) and **551306** (`FUN_0049e2e0`); 551302's *(decompiler detail omitted)* selects between them, and
   param 1 = near / 0 = far is `re-water.md` §cit. 4's reading, confirmed here by the record selection at
   551329-551336. [V]
   **Corrected 2026-09-23 (cleanse):** §7.2 below downgrades near/far to [D] (§1.1); read this tag as [D].
2. Both are one-line mode wrappers onto the engine's texture bind: `FUN_0049e240` (header 562392) →
   `FUN_0049ee68(tex, 0, 10, 0)`; `FUN_0049e2e0` (header 562442) → mode **8**. [V]
3. `FUN_0049ee68` (header 562988) hands texture + mode to `FUN_0049de40` (header 562247) at 563013, and that
   calls the TEX0 builder **`FUN_0049c5c8` at 562295** (builder header 561661). This is the engine's ONE
   texture-state path — the same one `analysis/re-material-flags.md` §2 reads for world models (Fable-lead
   verified, session 12). [V]
4. **TEX0, the decisive word** (`FUN_0049c5c8` 562031-562040): unless the texture is 32-bpp, the word gets
   `| 0x400000000` at **562034** — **TCC = 1, RGBA** — and the CLD bit `0x2000000000000000` is assigned at
   562033 and OR'd in at 562040. **No site ORs bits 35-36 into that word, so TFX = 0 = MODULATE**; the one
   `0x800000000` in the function (562149) lands in *(decompiler detail omitted)* → *(decompiler detail omitted)*/*(decompiler detail omitted)*, the MIPTBP1 word, which
   leaves at 562184, while TEX0 (*(decompiler detail omitted)*) leaves at 562180-562181. [V]
   *That the word IS TEX0 is settled by its field layout, not assumed*: the same local takes PSM `0x1300000`
   = PSMT8 (561961) / `0x1400000` = PSMT4 (561964) in bits 20-25, a TW ladder `0x4000000..0x3c000000` (bits
   26-29), a TH ladder `0x40000000..0x3c0000000` (bits 30-33), then CBP `<< 0x25` (bit 37) and CLD (bit 61) —
   fields that exist in no other GS register. [D]
5. *The constant read as bytes, not as a decompiled literal* (self-check 4): at va **0x49c9f0**
   `ori $2,$0,0x8000` then **`dsll $2,$2,19`** = 2^34 = `0x400000000` (TCC); at 0x49c9f8 `ori $3,$0,0x8000`
   then `dsll32 $3,$3,14` = 2^61 (CLD); `or` at 0x49ca00 / 0x49ca08 / 0x49ca14 merges them. The 32-bpp branch
   is real in the machine code too (`addiu $2,$0,0x20` at 0x49c9d4, `beq $8,$2,0x49ca0c` at 0x49c9e8, and
   0x49ca0c builds CLD only) — Ghidra's degenerate condition at 562032 is that test, not an artefact.
   ELF PT_LOAD vaddr 0x160000 / offset 0x1000, so file offset = va - 0x160000 + 0x1000. [V]
6. **The GS records** (`FUN_0048fca0`, header 553328, lines 553341-553369; layout +0 ALPHA word, +8 TEST,
   +0xc prim flags, `re-water.md` §cit. 5): near main **0x69c390 = 0x4144 / TEST 0x50000 / 0x58**, far main
   **0x69c3a0 = 0x4144 / 0x51001 / 0x58**, speckle 0x69c3b0 = 0x4148 / 0x51001 / 0x58, ocean base 0x69c3c0
   and 0x69c3d0 = **0x4100 / 0x50000 / prim 0x18**, ocean foam 0x69c3e0 = 0x100144 / 0x50000 / 0x58. ALPHA
   byte 0x44 = (A=Cs, B=Cd, C=As, D=Cd) → Cv = ((Cs-Cd) x As >> 7) + Cd; 0x48 = additive by As; the 0x00 in
   0x4100 is Cs alone, and that record's prim flags lack the 0x40 ABE bit. **Neither near record alpha-TESTS**
   (TEST 0x50000 has ATE=0; 0x51001 is the ATST=NEVER / AFAIL=FB_ONLY "colour but no Z" form). [V]
7. **Which near record**: 551332 picks by `DAT_00522a80`, which the BF1 ELF holds as **0** and which occurs
   exactly once in the corpus (that read) — so the near layer is 0x69c390, blended AND Z-written. The
   alternate 0x69c380 (0x4144 / 0x51001 / 0x58, identical to the far record) is unreachable on the disc. [V]

## 2. The twin, opened rather than assumed (self-check 3)

The 2026-09-14 blind trial's two water failures were both an ocean/flat mix-up, so the BF2 discrimination is
read first: **`FUN_005755b0` (header 708249) calls `FUN_00575fd8` when `DAT_00661490 == 0`** (708257, also
gated on *(decompiler detail omitted)*) **and `FUN_00575670` otherwise** (708268, gated on
`DAT_0060beea`). `FUN_00575fd8` is the FLAT renderer and `FUN_00575670` the OCEAN one — by their records:
the first uses 0x76c160/170/180 (708758-708764) and the speckle 0x76c190 (708893, 708926), the second the
ocean base/foam pair 0x76c1a0+0x76c1c0 / 0x76c1b0+0x76c1c0 (708488, 708560), mirroring BF1 exactly. **This RE
reads the FLAT one.** [V]

- Binds: **708713** (`FUN_00584a10`, far, mode 9) and **708716** (`FUN_00584a70`, near, mode 7) →
  `FUN_00585580` (header 718365) → `FUN_00584618` (header 717645) at 718386 → **`FUN_00582c90` at 717709**
  (builder header 716966), whose TEX0 assembly at **717178-717187** is line-for-line BF1's, TCC=1 at
  **717181**. [V]
- Records (`FUN_0057a3a8`, header 710704, lines 710717-710745) carry the same words as BF1's, and the near
  selector `DAT_0060c200` reads **0** in the BF2 ELF (one occurrence in the corpus, a byte read). [V]

## 3. The absence claim, searched not read (self-check 2)

`lead_tmp/re/water-alpha/tex0_bit_census.txt` — every occurrence of the TEX0 bits in both corpora, by
enclosing function:

| bit | BF1 | BF2 |
|---|---|---|
| TCC=1 `0x400000000` | 7 hits / 4 fns: `FUN_0049c5c8` x4 (561763, 561901, **562034**, 562152), `FUN_0048c848` (551788, the water speckle), `FUN_002bf130`, `FUN_00499f38` | 32 hits / 21 fns, incl. `FUN_00582c90` x4 and `FUN_00577088` (709200, the water speckle) |
| TFX=1 `0x800000000` | 3 hits, **none in a water function** (`FUN_0049c5c8` 562149 is the MIPTBP1 word) | 12 hits / 9 fns, **none in a water function** |
| TFX=2 `0x1000000000` | 2 hits, none in a water function | 10 hits / 9 fns, none in a water function |
| TFX=3 `0x1800000000` | 0 | 2 hits, neither in a water function |

`lead_tmp/re/water-alpha/tcc_clear_census.txt` — the mask that CLEARS TCC (`0xfffffffbffffffff`, whose only
zero bit is 34): **BF1 has exactly ONE site in the entire game**, the water speckle's 551846; BF2 has 14
sites in 14 functions, exactly one of them a water function (709258, the same speckle). So no other water
draw on either disc can be hiding a TCC=0. [V]

The water's per-draw GS state otherwise reaches the GS through `FUN_0039c1a8` records (applied at BF1 551339;
applier header 388492), which carry ALPHA, TEST and prim flags only — a record has no TEX0 field. [V]

**What the search cannot cover:** VU1 microcode 0x515410 is not in the decomp (`re-water.md` §8), so "the
microcode does not override TEX0" is **[H]**, not [V] — a weak [H]: TEX0 is uploaded with the texture by the
texture manager, the record applier carries no texture field, and the disc's authored alphas match the blend
(§4).

## 4. The census — what the water sheets actually carry (measured this session)

`lead_tmp/re/water-alpha/texcheck` over `extracted/` (`disc_water_alpha.txt`): the `fx__ Effect("Water")`
`MainTexture` / `FoamTexture` of all twelve water maps, decoded with the port's own `mungefs::texture`, mip 0.

| map | sheet | bpp | disc alpha min..max (mean) | below 255 |
|---|---|---|---|---|
| BF1 + BF2 KAM1 | `kam1_water_2` (ocean BASE) | 8 | 255..255 (255.0) | **0 %** |
| BF1 + BF2 KAM1 | `kam1_water` (ocean FOAM) | 8 | 0..218 (62.2) | 100 % (11.1 % at 0) |
| BF1 KAS1 | `kas1_water` | 4 | 142..255 (171.7) | 89.7 % |
| BF1 KAS2 | `kas2_water` | 4 | 96..96 (96.0) | 100 % |
| BF1 + BF2 NAB2 | `nab2_water` | 8 | 89..190 (107.5) | 100 % |
| BF1 + BF2 YAV1 | `yav_water` | 8 | 96..96 (96.0) | 100 % |
| BF2 KAS2 | `kas2_water` | 8 | 158..158 (158.0) | 100 % |
| BF2 DEA1 | `dea1_water` | 8 | 137..240 (182.5) | 100 % |
| BF2 FEL1 | `fel1_water` | 8 | 80..171 (117.3) | 100 % |
| BF2 DAG1 | `dag1_water` | 8 | 138..203 (172.1) | 100 % |

**Every water sheet is 4- or 8-bpp paletted**, so every one takes §1.4's TCC=1 branch and the 32-bpp
question below never touches water. **Ten of the twelve `MainTexture`s carry real alpha**; the other two are
the ocean base `kam1_water_2` on the two KAM1s, whose record does not blend — so 12 of the 14 sheets carry
alpha. (Corrected 2026-09-23 by WA's landing, lane 1: this line first said "eleven of the twelve", counting
the two KAM1 bases as one row; the landing's `maps` rebake kept exactly 12 sheets, one per wet map.)

Speckle frames (`disc_speckle_alpha.txt`): BF1's 25 `water_specularmask_NN` are **alpha 255 on every texel**
— exactly what §7.1's TCC=0 predicts, since their alpha could not be read anyway. BF2's 25 carry 0..~200
(mean 55), but no BF2 map authors `SpecularEnable`, so BF2 draws no speckle at all (`re-water.md` §1). Both
KAM1s' `water_specles0..3` are not in KAM1.LVL (they resolve elsewhere; not chased here).

## 5. The port question that came with the item

**"bf1_kam1's golden and all 8 walkabout panels were 0.000 although `kam1_water` flipped exactly like
bf2_kam1's — find the path that decides it"** (queue SM's report §For the lead 2). **It is not a water path
at all: it is the map's own terrain.** Measured with the port's `mungefs::terrain` over the disc
(`kam1_terrain_vs_water.txt`):

- **BF1 KAM1 ships a REAL heightfield** (`Terrain::is_stub` false; 256x256 at 40 m, world Y -20.16..0.00),
  and at **every one of its 1,618 wet cell centres the terrain sits at y = 0.00 — exactly 8.00 m ABOVE the
  water plane at y = -8** (above 1618, below 0, off-grid 0).
- **BF2 KAM1's `tern` is a STUB** — no heightfield at all — so its ocean is the only floor its platforms have.

Same sheet, same plane, same 1,618 wet cells: on BF1 the water is under the map's own ground everywhere, on
BF2 it is the visible sea. The committed goldens show it — BF1 KAM1's "sea" is a flat featureless grey plate
(the terrain), BF2 KAM1's is the dark water between the platforms. **Consequence for WA's build half: bf1_kam1
cannot move, bf2_kam1 can.** [V measured]

**Addendum (WA's landing, 2026-09-23, lane 1) — that terrain is textured with the FOAM sheet.** The baked
`bf1_kam1/terrain_kamino1.glb` names exactly one material, `kam1_water` (the `tern`'s LTEX layer 0, OPAQUE),
over world Y -20.16..0.00; the ocean `water.glb` names `kam1_water_2` at y = -8 [V measured, the GLB JSON].
So once WA gave `kam1_water` its disc alpha, bf1_kam1's golden kept its RGB byte-identical and moved only its
stored FRAME ALPHA, on 7.632 % of pixels (255 -> 0..197, median 60, tracking the sheet's own 0..218 / mean
62.2): the terrain draws through the port's opaque pipeline (blend None), which writes the texel's alpha into
the target while ignoring it for colour. It is NOT the ocean's foam draw — that pipeline blends alpha OVER
and cannot lower a destination alpha, and it sits 8 m under this heightfield. Nothing in the port reads
destination alpha (0 `DstAlpha` factors under `engine/crates/`), so it is invisible in play. [H], unread:
whether the PS2's terrain pass takes LTEX layer 0's texture alpha at all — if it did, BF1 Kamino's
sea-terrain would show the -8 m ocean through it.

**Addendum (KOC's RE, 2026-09-28, lane 2, [O]) — the [H] above is answered, and it is moot: the PS2 draws no terrain on
BF1 KAM1 at all.** See `analysis/re-terrain-enable.md`.
- BF1 KAM1 and BES1 author `DomeInfo { TerrainEnable(0) }` (fnv 0xae291b6d). BF1's DomeInfo parser `FUN_002e29e0`
  turns it into `… ELF 0x2e2c24–0x2e2d3c).
- Every terrain draw tests that flag first (`FUN_00487ad8` 549130; the far-scene call, `FUN_002f2308` 280296), and the
  height query answers −1000 (`FUN_00473710`).
- The PS2's RAM holds 0 on these two maps only. Patched save states at F9's pose: the sea is the ocean draw alone, over
  black.
- **In code the answer is yes:** a terrain pass would take the texture alpha.
  - The bind: `FUN_004857b8` 548047 binds in mode 3 → `FUN_0049c5c8` sets TEX0.TCC = 1 (562034).
  - The pass: ALPHA 0x44 with ABE; a one-layer patch's vertex alpha is 0x80, so As = At.
  - But no map that names `kam1_water` runs a terrain pass.
- WA's "7.632 % of pixels" frame-alpha movement above was the port's own plate.
- One Opus verifier: 26/26 SUPPORTED.

## 6. What stays open

- **Av, the per-vertex alpha, is still the VU1 table's** — `re-water.md` §3's Min.a→Max.a LUT (551893-551897,
  "[V table, D blend]") and §8. This RE closes the TEXTURE half of HOLD 21 and leaves the vertex half exactly
  where it was. [H] unchanged.
  **Corrected 2026-09-23 (cleanse):** for the FLAT water only — the ocean's per-vertex alpha is read off the EE
  in `re-water.md` §3a (the foam byte, halved); the 16-entry LUT is flat-water only.
- **32-bpp textures**: on the branch at 562037-562038 (BF1) / 717184-717185 (BF2) the TEX0 word gets CLD but
  NOT `0x400000000`, i.e. **TCC = 0 at that site**, while `re-material-flags.md` §2 states the 16/32-bit path
  ORs `0x400000000` in every TH case (717400-717470). Both can be true — they are different sites in the same
  builder — but the doc's blanket "TCC=1 for every texture in both games" is looser than the code. No water
  sheet is 32-bpp, so nothing here turns on it; queued as **TCC32**.
- The ocean's two-texture bind (`FUN_0049ec00` 551047 / `FUN_0049ea28` 551127, base + `DAT_005648f4`) was not
  walked into; it is covered by §3's clear-census, which finds no TCC=0 on that path, but which of the two
  units each record reads is unread. [H]

## 7. The verifier's one NOT SUPPORTED, reproduced by the lead before it was accepted

1. **The near SPECKLE's texture alpha does NOT reach the GS.** `FUN_0048c848` (header 551627) sets TCC=1 at
   551788 and then **clears it again** at **551846**: *(decompiler detail omitted)* — a mask
   whose only zero bit is 34. Lead-reproduced from the machine code at va 0x48ce5c-0x48ce90 (`lui $4,0xbfff;
   ori $4,$4,0xffff; dsll $4,$4,4; ori $4,$4,0xf` builds the mask, then `ld $2,0((cost omitted)); and $2,$2,$4;
   sd $2,0((cost omitted))` applies it to the first qword = TEX0). BF2 twin `FUN_00577088` (header 709073): 709200 then
   **709258**. The merged state is what the draw uses, and the choice is gated: BF1 551473
   `if (DAT_00522a64 != 0)` builds it and 551480-551486 then passes *(decompiler detail omitted)* (merged) instead of
   *(decompiler detail omitted)* (bound) — **`DAT_00522a64` reads 1 in the ELF and occurs exactly twice in the corpus, both
   reads**; BF2's `DAT_0060c1e4` likewise. The FAR speckle keeps the bound states (551438, 551449) and so
   keeps TCC=1. Each speckle builder is called from exactly one site in its corpus (551474 in `FUN_0048b9c0`,
   708921 in `FUN_00575fd8`). **This is what the port's `[GOLDENS]` speckle item W3 must copy: the near
   cross-faded speckle is RGB-only.** [V][O]
2. Smaller corrections folded in above: the TEX0 guard is "not 32-bpp" rather than "4/8-bpp" (§1.4); 562033
   and 562038 ASSIGN the CLD bit, with the OR at 562040; `FUN_0049c5c8` has a second TEX0 path (561737-561944)
   that bakes TCC=1 into its TH constants; near/far is [D] (§1.1); both games carry an unreachable alternate
   near record (§1.7); `FUN_0049ee68` drops its own *(decompiler detail omitted)*.
3. Everything else in the claim list was SUPPORTED with the enclosing function named: the bind chain on both
   discs, the TEX0 bits and their ELF bytes, all twelve record words per game, both selectors' ELF values, the
   flat/ocean discrimination, and the "no TFX bit in any water function" half of the absence claim.
