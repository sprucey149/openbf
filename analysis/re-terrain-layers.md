# OPUS RE (loop lane 2, 2026-09-28) - [O] UNAUDITED: no Fable pass
# RE: the terrain's texture LAYERS — `ALPH` is a per-vertex map of 4-bit "over" alphas; both games blend every painted layer (card TALPH)

> **AUDIT 2026-09-28 (loop lane 2, audit of the TALPH landing, OK — `engine/artifacts/loop/audits.md`).** This doc
> held under a re-derivation from the B1/B2 decomp, the ELF bytes and the disc, done before reading it. An independent
> decoder reproduced §1's census to the patch on all 27 terrains, and one Opus verifier found 14/14 SUPPORTED (the port's
> decode, bake and blend state included). Registering the layer textures against ray-cast PCSX2 ground pixels confirms
> §3's mapping on the PS2: S → u, T → v, the grid-0 anchoring and the SCAL scale (HOT1 AXIS 0, 4 views; KAS1 AXIS 6
> mirrored). §9's S/T item is closed. F6's box lies 128–192 m out, past the PS2 camera's 100 m far plane, so F17 and
> HOT1 are the stronger evidence. Scripts and sheets: `engine/artifacts/lead_tmp/audit-talph/`.

**The question** (card TALPH). `re-terrain-lighting.md` §6 (TERL) read the nibble table and the skip rule and left the
rest open. The port draws only LTEX[0]. This RE covers:
- what `ALPH` holds, and the order and blend of the layers;
- how each layer's texture is projected (SCAL / AXIS / ROTN);
- what the cutter-replaced (`PTCH`) patches do;
- two side questions: does the skip rule remove BF1 KAM1's y = 0 sea (card KOC)? Is TERL's Hoth residual the layers?

**Method.** The loop's RE discipline: the hops below, then the five self-checks, pasted verbatim in
`engine/artifacts/lead_tmp/re/talph/checks/` (`fn.txt`, `absent.txt`, `word.txt`, `writers.txt`, `callers.txt`).
Then one foreground Opus verifier, given the claim list only (`talph/claims.md` → `talph/verify/VERDICTS.md`, §10).

**Scratch, disc scripts and images:** `engine/artifacts/lead_tmp/re/talph/` (run the scripts from there).
- `alph.py`: the decoder library, `census` and `dump`.
- The other scripts: `semantics.py`, `ring.py`, `hidden.py`, `repl.py`, `texalpha.py`, `seams.py`, `holes.py`,
  `probe.py`, `topdown.py`, `tagscan.py`; each has a `*.txt` output.

**Conventions.**
- B2 = `analysis/SWBFRNT2.ELF-decomp.c`, B1 = `analysis/SWBFRONT.ELF-decomp.c`.
- "Asm" = `scripts/tools/eedis.py`; dumps are in `talph/ee_*.txt`.
- **P** = the render-patch edge in cells: tern `INFO` i16 at +18, 8 on every shipped terrain. A patch has (P+1)² = 81
  vertices.
- A **nibble** is one 4-bit weight. The **mask** is a `PTCH` `INFO`'s u16 at 0: bit k set = LTEX[k] painted in the patch.

## 0. The answer

| | BF1 | BF2 |
|---|---|---|
| **weights** | `ALPH`: per patch, per painted layer, (P+1)×(P+1) nibbles | same |
| **blend** | one pass per layer, ascending LTEX index; GS ALPHA 0x44 = (Cs−Cd)·As+Cd; As = table[nibble] | same |
| table | {0, 9, 18, 27, 36, 45, 54, 64, 73, 82, 91, 100, 109, 118, 128, 0}: 14 → 1.0, 15 → 0 | same |
| **0xf ("absent")** | a triangle with a 0xf corner is not drawn; nor is an all-0 triangle | same, except the patch's TOP layer, drawn whole (0xf → alpha 0) |
| 1-layer patch | no `ALPH` bytes; the builder fills 0xee (opaque) | same |
| **mask-0 patch** | no layer pass at all | same |
| **layer texture** | SCAL = metres per repeat; AXIS = projection (top-down, x-height or height-z, ±mirror, transpose); ROTN unread | same |
| cutter patches | own `ALPH` / `COLR`, one nibble / u16 per `POSI` vertex; AXIS ≥ 3 taken as top-down | same |

- **The weights are "over" alphas, not a partition.**
  - At 97–100 % of multi-layer vertices, the lowest present layer is 14 (opaque). The exceptions are BF1 KAS1, KAS2 and
    RHN2, where the rest are mostly holes, and BF2 KAS2 at 92 % (§5.2).
  - Higher layers paint over it, often opaque too. The alphas sum to 128 at only 24–91 % of vertices.
  - So C ← C·(1−a_k) + T_k·a_k for k ascending: exactly what the GS blend computes.
- **What the port throws away is every layer above LTEX[0].** Measured against the PS2 frames in §6:
  - F6 (BF1 KAS2's pale slope): PS2 (127,114,86), the layer model (116,103,81), the port (46,45,27);
  - F17 (KAS1's green ground);
  - The maintainer's BF2 KAS2 beaches: a pale-sand layer along every shore.
- **Mask-0 patches are never drawn by a layer pass**, and many maps have them: BF1 KAS1 951 of 1024, END1 872, RHN2 858,
  NAB2 855, KAS2 403, BES2 389; BF2 END1 872.
  - They lie at each map's floor height, under the lakes and sea or outside the play area.
  - The port draws them with LTEX[0]. That is F16's grey lake bed, where the PS2 shows no lake bed (§7).
- **KOC: no.** BF1 KAM1's 1024 patches are all single-layer. The skip rule never fires, and the y = 0 sea terrain is
  drawn (§7).
- **TERL's Hoth residual:**
  - on BF2 HOT1 it is not the layers: TERL's boxes paint hoth_main_1 alone;
  - on BF1 HOT1 the layers are part of it: hoth_main_4 is the visible layer there (§7).

## 1. The `ALPH` stream [V][O]

1. **Where it lives.** `tern → PCHS → ALPH`, after the `PTCH`s and `COLR` (`census.txt`).
   - The PCHS parser (B2 FUN_0056bbf0, B1 FUN_00481608) walks it once, in `PTCH` file order (B2 703321–703334, B1
     546026–546045):
     - entry +8 is the mask;
     - FUN_0056b9c8 (B1 FUN_00481328) = popcount(mask & 0xffff), 703103–703114.
   - An entry with **popcount > 1** stores the current pointer at entry +0 (703323). It then consumes
     (P+1)·(P/2+1)·layers **decoded** bytes (703325).
   - Entries with 0 or 1 layers take no pointer.
   - P = DAT_00661550 (B1 DAT_0056485c). Its only writer is FUN_0055db58 694906 (B1 FUN_004726b8 536767).
2. **The codec** (B2 FUN_00572c98 707026 / 707043; B1 FUN_00488850 549637–549697). Control byte c:
   - c < 0x80: copy c+1 literal bytes;
   - c ≥ 0x80: repeat the next byte (c−0x80)+1 times.
   - This is `mungefs::rle` with elem 1. `COLR` uses the 2-byte twin (FUN_00572d38 / FUN_00488910).
3. **Per patch** (B2 FUN_0056e898 704627–704721, B1 FUN_004852e0 547873–547947):
   - 0 layers: nothing is built (704627);
   - 1 layer: the buffer is filled with **0xee** (704680; ELF 0x56eb00 `addiu v0,zero,238`; B1 547912, 0x4854c0);
   - ≥ 2 layers: `ALPH` is decompressed for layers × (P+1)(P/2+1) bytes (asm 0x56ea64 `mult a2,a0,v0` = 45 × layers,
     the call at 0x56eae0).
4. **Layout.**
   - Per-layer blocks are consecutive, in ascending layer bit. The builder's pointer advances by (P+1)(P/2+1) per painted
     layer (704721, B1 547947).
   - A block is P+1 **rows** of P/2+1 bytes. Row r is z = z0 + r; column c is x = x0 + c. Both are added to the patch
     origin in the height lookups of FUN_0056c9d8.
   - Vertex c is the **low nibble of byte c/2 for even c, the high nibble for odd c** (703766–703783; FUN_0056e898
     704742–704771). The tenth nibble of each row is padding.
   - Verifier (A5): the in-row unpack is hard-wired to 5 bytes, so the format as read works only for P ≤ 8. Every shipped
     terrain has P = 8.
5. **Shared edges.** A patch's column / row 8 is its neighbour's 0. Each patch carries its own copy, and the copies
   disagree on 0–3.3 % of edge pairs (§5.4).
6. **On the disc** (`python3 alph.py census` → `census.txt`). On all **27** terrains (17 BF1 + 10 BF2), the walk ends
   exactly at the chunk's end, and no patch's decode overshoots. The munger restarts the RLE per patch.

## 2. The draw [V][O]

1. **Order and blend.**
   - FUN_0056e898 builds one chain per painted layer.
   - FUN_0056f208 draws chains 0..15 in ascending order and skips null ones: *(decompiler detail omitted)* 705213, loop
     705209–705442. B1 FUN_004857b8 does the same, 548043–548076, texture DAT_00754638[k].
   - Every pass uses GS ALPHA **0x44** = (Cs−Cd)·As + Cd:
     - BF2 descriptor 0x76c080 (705409; TERL §1.5). The plain branch is also gated by DAT_0060c1b4 = 1, never written
       (verifier). The lit top layer's multi-pass set starts with a copy of 0x76c080.
     - BF1 0x69c2e0, baked into each chain (TERL §3).
   - Each layer's colour is T_k × COLR: the same vertex colour for every layer (TERL §0).
2. **Alpha.**
   - As = table[nibble] (B2 DAT_0060c1a0, read at 703870 / 703926 / 703978 …; B1 DAT_00522a20, 546834 …).
   - Both tables are the 16 bytes above in the ELF (`word.txt`), with no writer: `writers.txt`, and no ELF store
     (`addrscan`).
   - Every multi-layer terrain texture is opaque (§5.3), so the texture's alpha never scales As.
3. **The skip rule** (B2 703796–703800 and 703983–703986; B1 546700–546711). A strip triangle is not emitted when any
   of its three nibbles is 0xf, or all three are 0.
   - **BF2 applies it only when argument 3 == 0.** FUN_0056e898 sets argument 3 = 1 exactly for the patch's highest painted
     layer (*(decompiler detail omitted)*, 704690–704708). That layer is drawn whole, with 0xf as alpha 0. It is the layer that carries
     the lit passes (TERL §5).
   - **BF1's builder FUN_004828b8 has no such parameter:** the rule applies to every layer.
   - The rule is visible. 16 % of 0xf vertices sit next to a non-zero weight (§5.2), so a skipped triangle cuts a painted
     layer off along a triangle edge. On its top layer, BF2 fades it to 0 at the 0xf corner instead.
4. **Triangulation** (703772–703783, 704168–704176).
   - Row r's strip emits (x−1,B), (x−1,A), (x,B), (x,A), … with A = r, B = r+1 on even r and A = r+1, B = r on odd r.
   - So the quad diagonal is **(x,z)–(x+1,z+1) on even z and (x,z+1)–(x+1,z) on odd z**, the height query's own split
     (`re-terrain-heights.md` §5).
   - The skip test's triangles are {(B,x−1), (A,x−1), (B,x)} and {(A,x−1), (B,x), (A,x)}.
   - The port's render mesh (`terrain_glb.rs`, `[a, c, b, b, c, d]`) uses the odd-row split on every row.
5. **Mask-0 patches.**
   - A new cache slot's 16 chain words are zeroed: B2 FUN_0056e7d8 704560–704566, B1 FUN_004851b0 547783–547786.
   - FUN_0056e898 builds nothing when the popcount is 0 (704627; B1 547875), so **no layer pass is issued.**
   - Verifier (B2, PARTLY): two other passes do not test the mask.
     - The DTLX detail pass (BF2 FUN_00570f68 → FUN_005700a8; BF1 FUN_00486e30 → FUN_00485f20; on by default) multiplies
       the frame by 2 × detail alpha (ALPHA 0x89) over the 3×3 patches round the eye, whatever their mask. That is card
       TDTL's pass.
     - BF2's 0x20000 pass draws the `LOWR` mesh instead of the patches (706498–706499, FUN_00572458).

## 3. The per-layer texture coordinates [V][O]

1. **Scale** (703747; asm 0x56cb20–0x56cb70: `div.s`, `lui 0x4500` = 2048.0, `lui 0x3f00` = 0.5, `cvt.w.s`; B1 546641):
   k = (int)(gridUnit / SCAL[k] · 2048 + 0.5) ST units per grid cell.
   - SCAL[k] is DAT_0083a900[k] (694846; B1 DAT_007546b8, 536715–536720).
   - AXIS[k] is the byte DAT_0083a830[k] (694774; B1 DAT_007545e8, 536989).
   - Both are passed per layer at 704703–704709 (B1 547921–547931).
2. **2048 ST units = one repeat.**
   - FUN_00494af8 unpacks the ST stream as V2-16 **signed**, STMOD 1 (offset), onto STROW 0x45808000 = 4112.0 (553800,
     553823–553831; ELF 0x494df4–0x494e38).
   - VU1 subtracts 4112 before × Q: `LOI 0xc5808000` at VU 0x04a (.vutext + 0x260, both ELFs), `ADDi.xy vf3, vf2, I` at
     0x051.
   - The terrain's pass flags 0x1000 route there: 0x162 → 0x2a3 → 0x2da → 0x192 → 0x00d → 0x031 (verifier C5).
   - **So SCAL is world metres per texture repeat**, the reading `mungefs::terrain` labels PROVISIONAL.
3. **AXIS** (703733–703758; asm 0x56cac4–0x56caf8 `slti 6` / `addiu −6` / `slti 3` / `addiu −3`; B1 546628–546633):
   - a ≥ 6: S is negated, and a −= 6. Then a ≥ 3: S and T are swapped, and a −= 3.
   - a == 0: S = x·k, T = z·k (top-down).
   - a == 1: S = x·k, T = h·k.
   - a == 2: S = h·k, T = z·k.
   - h = raw height × heightScale / gridUnit × k (703830; DAT_00661494 = 1/gridUnit, 694959), **truncated** (verifier C3).
   - x, z are the GRID indices: world x = (x − n/2)·gridUnit, world z = (z − n/2 + 1)·gridUnit.
   - Whole repeats are subtracted per row, per patch and in a patch-wide re-centre (`& 0xfffff800`, 703808 / 703814,
     704181–704218). None of it is visible.
   - AXIS % 3 also picks the layer's texture-state hash (the mip mode, `re-texture-wrap.md` §4): FUN_00571bd0 706527 /
     706534, B1 FUN_00487ad8.
4. **ROTN is read by neither game.** No immediate and no data word for 0x4e544f52 in either ELF; the same scan finds
   SCAL, AXIS, ALPH and DTEX (`talph/tagscan.py`; `absent.txt`). BF2 DAG1's ROTN 1.0 does nothing.
5. **DTEX.**
   - BF2 writes DAT_0083a8c0 (694746) and never reads it: 0 accesses by addrscan; the one pointer materialisation is the
     store's.
   - BF1 reads its DTEX table only in the draw's texture preload loop (FUN_00487ad8 549166) [D: never bound to a pass].

## 4. Cutter-replaced patches [V][O]

- **Weights and colour.** B2 FUN_0056dcd8 (B1 FUN_00483af0) walks the `STRP` indices.
  - Vertex i (`POSI` order) takes nibble i of the `PTCH`-level `ALPH` block, low nibble first; a block is (count+1)/2
    bytes per layer (704324–704327).
  - It takes u16 i of the `PTCH` `COLR` (704385).
  - The same skip rule applies (704354).
- **AXIS is compared raw with 2 and 1 only** (704363 / 704371; asm 0x56e094–0x56e120 `bne fp, 2` / `bne fp, 1`).
  - Values ≥ 3 get the plain top-down map: no mirror and no transpose, unlike the neighbouring grid patches.
  - This is live on the disc: 5 BF1 RHN1 and 12 BF2 GEO1 replacement patches paint AXIS ≥ 3 layers (verifier).
- **On the disc** (`repl.py` → `repl.txt`): 122 replacement patches on 13 terrains.
  - `COLR` decodes to count u16 on all 122, count = `POSI` bytes / 6.
  - `ALPH` decodes to ((count+1)/2)·layers on the 104 multi-layer patches.
  - The 18 single-layer patches carry an empty `ALPH`; the builder fills 0xee.

## 5. The disc [V measured]

### 5.1 Census (`census.txt`)

| map | patches | mask 0 | multi-layer | LTEX (painted) | AXIS ≠ 0 |
|---|---|---|---|---|---|
| BF1 BES1 / KAM1 | 1024 | 0 | 0 | 1 (kam1_water) | — |
| BF1 BES2 | 1024 | 389 | 4 | 2 | — |
| BF1 / BF2 END1 (same data) | 1024 | 872 | 75 | 6 (5) | — |
| BF1 GEO1 | 1024 | 0 | 396 | 6 | L4 = 5, L5 = 1 |
| BF1 HOT1 / BF2 HOT1 | 1024 | 0 | 889 / 901 | 3 | — |
| BF1 KAS1 | 1024 | 951 | 70 | 5 | L2, L3 = 6 |
| BF1 KAS2 | 1024 | 403 | 76 | 4 | L3 = 3 |
| BF1 NAB1 | 1024 | 0 | 352 | 3 | — |
| BF1 NAB2 | 1024 | 855 | 0 | 1 | — |
| BF1 RHN1 | 1024 | 0 | 108 | 6 | L3 = 9, L5 = 3 |
| BF1 RHN2 | 1024 | 858 | 66 | 5 | — |
| BF1 TAT1 / TAT2 | 1024 | 0 | 846 / 62 | 6 / 5 | — |
| BF1 / BF2 YAV1 (same data) | 1024 | 0 | 70 | 6 | L1 = 1 |
| BF1 YAV2 | 1024 | 0 | 22 | 4 | — |
| BF2 DAG1 / FEL1 / KAS2 | 256 | 0 | 66 / 51 / 111 | 4 / 4 / 3 | — (DAG1 ROTN L0 = 1.0, unread) |
| BF2 GEO1 | 1024 | 0 | 110 | 9 | L4 = 1, L5 = 5, L8 = 3 |
| BF2 NAB2 / POL1 / TAT2 | 256 / 256 / 64 | 0 | 0 / 19 / 21 | 1 / 5 (4) / 3 | — |

- At most 9 layers are painted on a map (BF2 GEO1) and 8 in a patch.
- BF1 TEST (not in the menus) is left out of the table.

### 5.2 What the nibbles mean (`semantics.txt`, `ring.py`, `hidden.py`)

- **The lowest present layer is opaque** (14) at ≥ 97 % of multi-layer vertices on every map except BF1 KAS1 (67 %),
  KAS2 (73 %), RHN2 (82 %) and BF2 KAS2 (92 %). On the three BF1 maps the rest are mostly vertices where no layer is
  present (holes, §5.5).
- **Not a partition.** The alphas sum to 128 at 24 % (BF1 KAS1) to 91 % (BF2 TAT2) of vertices. "ee", two opaque layers
  at one vertex, is common, and the top one wins.
- **0xf is "absent", not "hidden".**
  - Of all 0xf nibbles, 31 % sit under an opaque higher layer, 47 % are on the patch's top layer, and 22 % are neither.
  - 16 % of 0xf vertices border a non-zero weight in the same layer (`ring.py`: 45,903 of 293,116), so the triangle
    skip changes pixels.

### 5.3 Texture alpha (`texalpha.txt`)

- Every LTEX texture of every terrain on both discs is fully opaque, except **kam1_water** (BF1 KAM1, alpha 0..218).
- BES1 names kam1_water too, but BES1.LVL does not carry the texture.

### 5.4 Shared edges (`seams.py`)

- `COLR` agrees on every shared-edge vertex of every map.
- The `ALPH` alpha differs on 0–3.3 % of edge pairs: BF1 HOT1 1,236 of 38,106, BF2 HOT1 1,288 of 38,844, mostly by ≥ 36
  of 128. These are layers that stop at a patch border.
- **Per-patch vertices are needed to reproduce the PS2 exactly.**

### 5.5 Mask-0 patches and holes (`holes.py` → `holes_*.png`; `topdown.py` → `topdown_*.png`)

- **Mask-0 patches** sit at the floor height: BF1 KAS1 −52.2 m, KAS2 −5.9 (the lake bed), BES2 −50, NAB2 −50, RHN2 −60,
  END1 0 to −3.8.
- **Holes** are vertices where the layers cover < 0.5: KAS1 1,533, KAS2 1,282, RHN2 686, END1 40, BF2 KAS2 51. They lie
  at the same heights, as rings round the islands.
- **Top-down composites** of the layer means: `topdown_{bf1_kas1, bf1_kas2, bf2_kas2, bf2_hot1}.png`.
  - Left panel: the PS2 rule. Navy = mask-0 (not drawn); red = a hole.
  - Right panel: LTEX[0] everywhere, the port today.

## 6. The PS2 frames against the model [V measured]

**The instrument.** `probe.py` raycasts a box's pixels to the heightfield at the library pose. At each vertex it
composites the painted layers' MEAN texture colours by §2's rule, then applies COLR at TERL's per-game scale.
- It is a coarse check of the model: mean colours, no skip rule, no detail pass.
- The port's numbers come from `engine/artifacts/pcsx2/ref/FINDINGS.md`.

| pose (box) | PS2 | layer model | the port today |
|---|---|---|---|
| BF1 KAS2 `cp4_cp11_dock_two_spawn` (540,110,630,150), F6 | (127,114,86) | (116,103,81) | (46,45,27) |
| BF1 KAS1 `cp2_cp2_eastharbor_spawn` (0,260,150,330), F17 | (59,47,26) | (62,50,26) | grey sand (kas1_main_2) |
| BF1 KAS1 `cp2_cp2_eastharbor_spawn` (420,330,640,420) | (51,41,20) | (56,51,30) | grey sand |
| BF1 HOT1 `cp2_cp6_postA_spawn` snowR (430,300,640,420), luma | 197.3 | 214.4 | 228.1 (TERL §9's rule) |

- **Dock Two:** every sampled vertex has layer 0 (kas2_main_1, dark grass) absent (0xf) and layer 1 (kas2_main_2, pale
  sand) opaque.
- **KAS1:** the sand base is overlaid by kas1_main_1 / _4 (greens) at 0.42–0.71.
- **BF2 KAS2 (the maintainer's beaches), `topdown_bf2_kas2.png`:** kas2_main_2 (pale sand, mean 135,131,120) runs along every shore
  and round the island, over kas2_main_3 (sandy brown) in the shallows. The port draws kas2_main_1 there.
  - No library pose shows a beach: all five are on the platform. The build captures one.

## 7. Answers for other cards

- **KOC: TALPH does not fix it.**
  - BF1 KAM1's 1024 masks are all 0x1 and its `ALPH` is empty, so every weight is 0xee and no triangle is skipped. The
    y = 0 sea terrain is drawn, textured with kam1_water.
  - KOC's own question stands: kam1_water's alpha 0..218 through TEX0.TCC.
- **TERL §0's [H] (the Hoth residual is the layers): refuted on BF2, partly true on BF1.**
  - **BF2 HOT1.** Every sampled vertex under TERL's four LITE boxes (`cp1_cp6_postA_spawn` / `_v2`, near / mid) paints
    hoth_main_2 / _4 at 0 or 0xf. The ground there is hoth_main_1 alone, so BF2's residual (the rule 223 / 221 against
    the PS2's 210 / 206) has another, open cause (TERL §11 note).
  - **BF1 HOT1.** TERL's four boxes (`cp2_cp6_postA_spawn` / `_v1`, snowR / snowL) sample 2–3 vertices each. Their
    weights are "fe" / "ee" over [hoth_main_1, hoth_main_4], so the visible layer is hoth_main_4 (mean 214, against
    hoth_main_1's 225).
    - The layer model gives 214.4 in all four boxes. At the spawn, TERL's LTEX[0] rule is 228.1 / 223.0 (TERL §9).
    - The PS2 shows 197.3 / 191.4 (spawn) and 194.0 / 212.2 (v1).
    - So at the spawn the layers close 45 % (snowR) and 28 % (snowL) of the gap; the rest is open.
- **F16 / REFL.**
  - The lake beds of BF1 KAS1 and KAS2 are mask-0 patches, so the PS2 draws no lake bed. That is why its water shows the
    mirrored sky shells (card REFL).
  - The port draws the bed with LTEX[0].
- **TDTL.** The detail pass also covers mask-0 patches near the eye (§2.5).
- **Side note (FOLG / grass).** FUN_0056fae0 scatters per-layer "detail objects" within 100 m of the eye.
  - It places them on flat ground (normal y > 0.96), at the vertex's dominant layer: the highest layer whose weight is
    8..14 (FUN_0056e898 704742–704771).
  - Its model table DAT_0083a940 has no writer, in the text or by ELF scan [H], so it is dead on the shipped game.

## 8. What the port owes

**TALPH (build).** Draw what the PS2 draws on every patch that paints ≥ 1 layer.
- **Bake.**
  - Per render patch, (P+1)² own vertices: the per-patch weights differ on shared edges (§5.4).
  - Per vertex: every painted layer's alpha = table[nibble] / 128.
  - Per layer: the triangles that survive §2.3's rule. On BF2 the top layer takes all of them.
  - The PS2 split (§2.4).
  - Replacement patches take their own `ALPH` / `COLR` (§4).
  - Keep the n×n grid block that `game::assets::terrain_grid_size` reads, or move the sim's heightfield source; the sim
    must not move.
- **Render.** Layers in ascending LTEX index, each T_k × COLR (TERL's `LightPath::Terrain`), blended over the previous by
  its interpolated alpha. The per-layer S/T come from §3: SCAL[k], AXIS[k] (mirror / transpose / height axes), and the
  grid indices.
- **Where the layers cover < 1** (holes, §5.5), keep today's look as a documented backstop: LTEX[0] × COLR under the
  layers. Mask-0 patches also keep today's LTEX[0] draw. Both go in TMSK.
- **Moves.** Every map golden with a heightfield, plus `tat1_*`; `rebake = tat1, tat2, maps`. The heightfield is
  unchanged, so sweep / arena / sim stay.

**TMSK (new, build; deps REFL, CLRB).** Drop the mask-0 patches and the TALPH backstop. The PS2 draws no terrain there,
and what shows through the lakes is REFL's mirrored sky. Without REFL and CLRB the gap would show the clear colour.

## 9. What stays [H]

- What BF2's 0x20000 render pass is (it draws `LOWR` instead of the patches).
- DAT_0083a940's writer (the detail-object table).
- Per-layer clamping: the PS2 clamps T·c at 255 per layer, then blends in 8 bits. A port that blends and then multiplies
  differs only where BF1's c > 1 meets a bright texture.
- Whether BF2's top layer writes Z at its alpha-0 triangles. That depends on reading the descriptor's 0x50000 word as
  the GS TEST: Z on, GEQUAL, no alpha test. Neither the reading nor the effect has been measured.
- S → u, T → v in the port's texture convention; to be checked against a frame of kas2_cobbles_02 (AXIS 0) and a
  mirrored layer (KAS1 AXIS 6). **Closed by the audit (2026-09-28):** checked by texture registration against the
  PCSX2 frames. HOT1 hoth_main_4 (AXIS 0): the claimed mapping and phase rank 1st of 48 mappings and 1st of 64 phases
  on 4 views. KAS1 kas1_main_4 (AXIS 6): mirrored 1st, unmirrored 39/48 (`lead_tmp/audit-talph/uvreg.py`).
- The Hoth residual beyond the layers: all of BF2's, and 55–72 % of BF1's at the spawn (§7).

## 10. Verification

**Self-checks** (`talph/checks/`, verbatim):
- **fn:** all 108 cited lines are in the named functions (16 functions).
- **absent:**
  - ROTN: 0 hits in either corpus, and 0 immediates / 0 data words in either ELF (`tagscan.py`, validated on four
    sibling tags).
  - DAT_0083a8c0: its only hit is the store; addrscan finds 0 accesses.
  - The alpha tables' readers are the two builders only.
- **word:** the alpha tables; the builder's AXIS decode, 2048.0, 0.5 and −2048; the 0xee fills (both games); the
  replacement path's raw compares; FUN_00494af8's STROW / STMASK / UNPACK words; the VU1 `LOI 0xc5808000` at
  .vutext + 0x260 (both ELFs).
- **writers:** DAT_00661550, 00661558, 00661494 and the BF1 twins each have one writer, FUN_0055db58 / FUN_004726b8. The
  tables have none.
- **callers (the twin walk):** each function in both chains has exactly one caller (`callers.txt`).
  - BF2: FUN_00571bd0 → FUN_0056f208 → FUN_0056e898 → FUN_0056c9d8 / FUN_0056dcd8.
  - BF1: FUN_00487ad8 → FUN_004857b8 → FUN_004852e0 → FUN_004828b8 / FUN_00483af0.

**The verifier** (one Opus agent, claim list only; `talph/verify/VERDICTS.md`, 177 tool uses): **21 SUPPORTED, 3 PARTLY,
0 NOT SUPPORTED.** Folded in:
- A6: 27 terrains, not 29.
- D4: 122 replacement patches, not 86; the single-layer ones have an empty `ALPH`.
- B2: the detail pass and BF2's `LOWR` pass do not test the mask.
- C3: the height value is truncated.
- C4: the patch-wide re-centre.
- A5: P ≤ 8.
- AXIS % 3 picks a texture state.
- C6 is live on 17 disc patches.
- BES1 has kam1_water's layout.
- B1: the DAT_0060c1b4 gate.
- Tooling: addrscan misses a `lui` set on another branch path, so every absence was also checked by an any-base
  immediate scan.


## Display-colour compositing correction (2026-11-04, issue #116)

The Dagobah comparison in `re-foliage.md` section 18 also exposed excess
brightness on the ground. The source-over rule established in section 2
operates on GS display bytes. The renderer now runs terrain overlay layers
through its compatible non-sRGB colour attachment, converts their shaded
output to display units and resumes the normal attachment before model,
skinned and lighting passes. Both ordinary and no-discard terrain layer
pipelines use this path. The base terrain keeps its existing depth writes.
Terrain and grass also filter an existing texture's display view before
colour decoding. The texture data and mip chains are unchanged.

A numeric GPU regression composites one and two half-alpha black terrain
layers over display grey 200; results must match 200*(127/255)^N within two
levels. The Dagobah matched-camera ground region changes from mean RGB
(51.51,64.04,65.28) to (49.03,61.39,61.64), versus the original
(45.97,57.02,55.96). This closes the linear blend deviation, not the
remaining complete-scene comparison.
