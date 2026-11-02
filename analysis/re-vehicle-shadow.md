# OPUS RE (2026-10-16) - [O] UNAUDITED

# RE: vehicle shadow volumes - how BF1 builds, counts and darkens them, and why BF2 draws none (card VSHD)

Question (card VSHD, from `re-chunk-census.md` 2-3): how the PS2 builds a model's `shdw` volume (FUN_004ad180 /
FUN_00592668), what light vector it uses, how the GS darkens (passes, colour, amount, BF2's `Intensity`), what `near`
changes - enough to build the faithful vehicle shadow.

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`, both ELFs, the BF1 VU1 main
program (`scripts/tools/vu1dis.py`), every disc `shdw` chunk, PCSX2 RAM of four save states and one live
PCSX2 poke run (BF1 NAB1). Tags: [V] read (line checked with `re_check.py fn`), [D] derived, [H] hypothesis, [M]
measured on the PS2. Load-bearing claims carry [O].

Hops: the piece's draw slot (B1 vtable 0x559340 +0xc = FUN_004ae6a8 -> FUN_004adbc8) -> its arguments (disassembly)
-> the rebuild FUN_004ad180 / FUN_004ad5c0 and the parser FUN_004acb20 -> the length FUN_004a8060 = DAT_00522b44 and
its four setters -> the render bucket 6 (FUN_0049e280) and its other users: the per-frame set-up in FUN_004a57e0 and
the per-frame tail in FUN_004a7098 -> the tail's display list built at load by FUN_00499f38 -> its TEX0 / TEXA words
(ELF + RAM) -> the VU1 pass flag 0x100 (0x17f -> 0x544) and the facing loop (0x620 -> 0x636) -> BF2's twins (the
piece vtable 0x6500d0, FUN_00581018, DAT_0060c310, DAT_007c5758) -> RAM of BF1 / BF2 states -> a live TEXA poke.

Tools (all under `engine/artifacts/lead_tmp/vshd/`): `tools/shdw_check.py` (every disc `shdw` parsed and its
half-edge mesh checked), `ps2/texa.py` (the live poke), `tools/texa_measure.py` (its score), `tools/onoff.py` (FXC's
pairs refit), `tools/b2pieces.py` (shadow pieces and their draw slot in a state's RAM), `tools/ram.py`, `tools/fnat.py`;
`checks/` (the re_check runs), `claims.json` (the claims as the verifiers got them).

## 0. The answers

| | BF1 | BF2 |
|---|---|---|
| drawn? | yes, every battle, one view and split screen: the gate DAT_00522ed0 is 1 in the ELF and its one store, a setter at 0x4ae850, is never called (`re-chunk-census.md` 2 [receipt:af3dc61d]) [V][O] | **no**: the piece's draw slot is an empty function, in the ELF and in the RAM of three battles [V][M][O] |
| volume | each `shdw` piece (a closed half-edge mesh, `NAME` = the node it follows) is rebuilt every draw against the sun direction (SunInfo, DAT_0069c590): the silhouette loops extruded along the sun by len; both caps only when "near" [V][O] | (rebuilt every draw, then not drawn) |
| len (how far) | class factor x the terrain hit fraction of a ray from the vehicle along the sun: flyers 500 m, hovers / walkers / droidekas 150 m [V][O] | - |
| near | the model's view depth < 30 m + its sphere radius: z-fail with caps; else z-pass without caps [V][D][O] | - |
| darkening | every pixel inside any volume (count != 0, mod 256) is multiplied by **0.625** (black at 48/128), in the frame buffer, after the opaque buckets and before water / far scene / translucents [V][M][O] | the same pass is built at load (TA0 0x30) and never called [receipt:02bc10f8] [V][O] |
| per map | a constant: … SunInfo `ShadowColor`'s setter is an empty function [V] | `Effect("Shadow") Intensity` is stored and never read [receipt:3dc255e4] [V][O] |

So the card's BF2 half (Intensity per map, off in split screen) has nothing to build: `re-chunk-census.md` 3 read the
gate right but the draw behind it is a stub (correction in 10).

## 1. The chunk [V][M][O]

`shdw` (inside `modl`; the model and its LOWD twin each carry one) = `NAME INFO SHDV SHDI` (B1 FUN_004acb20):
- `NAME`: the node the piece follows, looked up in the owning model by FUN_00496d08 and kept at geometry +0x10
  (569497-569503): `dummyroot` for most; the X-wing has 9 pieces: `dummyroot`, `xwing_b_gear2/3`, `xwing_bl_hatch3/4`,
  `xwing_br_hatch3/4`, `xwing_f_gear2/3` (the landing gear and the S-foil hatches move with their nodes).
- `INFO`: two u32, the vertex count and the SHDI short count (569506-569512).
- `SHDV`: three f32 per vertex (569524-569536), the model's frame.
- `SHDI`: groups of four u16 = one half-edge {vertex, next (same face), twin, 0xFFFF}; the loader copies the first three
  (569555-569574). Faces are the `next` loops; each gets a plane from the first non-degenerate cross product of its
  consecutive vertices (squared length in (1e-5, 1e6)) (569575-569660).
- **[M] Every disc chunk fits** (`tools/shdw_check.py`): BF1 240 chunks (73 piece names), BF2 164 (41), no error:
  twin(twin(h)) = h, twin(h) = h ^ 1 (twins are pairs 2k / 2k+1, which the silhouette walk relies on),
  vertex(next(h)) = vertex(twin(h)), every face loop closes without a revisit (the loader spins forever on one,
  569596-569600), the 4th short is 0xFFFF in all 62,508 groups. Faces of 3-20 sides; most pieces are one closed
  shell (Euler 2), some hold several (the K-wing 7). 6-486 vertices, 18-1742 half-edges.

## 2. The volume [V][O]

Per piece per draw, FUN_004ad760 hands FUN_004ae680 the node's matrix and the light vector FUN_0049b8a8() =
&DAT_0069c590 (570152-570154, 561128) - the SunInfo direction, the vector the light travels (`re-sun-direction.md`
1, the port's `sun_dir`). FUN_004ae700 -> FUN_004ad180:
- L_local = (L . row0, L . row1, L . row2) of the matrix; each face is lit when n . L_local < 0, the flag on its
  half-edges' byte 7 (569770-569802).
- Silhouette: the half-edge pairs (2k, 2k+1) whose two faces differ; FUN_004ad5c0 walks each closed silhouette loop
  (569829) into a list of {vertex, loop-start flag}, closing it by repeating the first vertex.
- Caps: … no writer [receipt:d7704603]) selects the faces with n . L_local > 0 - the faces turned AWAY from the sun
  (569886-569915).
- FUN_004adbc8 (570452-570590): per silhouette point two vertices, P and P + L_local * len, as a triangle strip per loop
  (16-bit positions in a 1024 m box: (v + 512) * 65535/1024, … 0x8000 on a loop's first pair =
  the strip restart). Only when `near` (570593) the two caps follow: the selected faces at their place (DAT_00522ef8)
  and the same faces moved by L_local * len (DAT_00522efc), each as a strip (both flags ELF 1, no writer [receipt:d7704603]).
- Every piece of the model is drawn each time (the list off the container, 570065).

## 3. len, the light, near [V][O]

- **len = DAT_00522b44** (FUN_004a8060, 570392): ELF 30.0, set before each vehicle's draw by FUN_004a8050 = factor x
  FUN_00474680(0.05, 5.0, a, b), the terrain-only segment test (1.0 when the segment misses, or when DAT_005229a8, the
  terrain switch, is 0):
  | caller | class (vtable) | factor | a (ray start) |
  |---|---|---|---|
  | FUN_001d8620 (84629) | flyer (0x53083c, 0x52f47c; EntityFlyer 0x5308f0) | 500 (0x43fa0000) | collision prim centre + (0, radius, 0) |
  | FUN_001ea2b8 (97127) | hover (0x531524; EntityHover 0x531540) | 150 | the same |
  | FUN_00217278 (128556) | walker (0x533a64) and command walker (0x53e1fc) | 150 | pos - L*d raised by (radius - d), d = leg-prim radius / (2 abs(L.y)) when L.y < -0.5, else the radius |
  | FUN_001cdfc0 (77157) | droideka (0x530084) | 150 | collision prim centre + (0, radius, 0) (no droideka model has a `shdw`) |
  b = a + L * factor. Each one then sets draw flag 0x10000 - the flag the model draw needs (`(flags & 0x230000) ==
  0x10000`, 557404) - and only when the model has a shadow container (+0xb8). No other decomp line sets 0x10000 in a
  draw's flags (text search); so `rep_bldg_forwardcenter`'s volume (a building) is never drawn [D/H: a flag built
  elsewhere would escape a text search].
- **near** = max(0, view depth of the model's origin) < DAT_00522ab0 (30.0) + its sphere radius (557221-557233, 557409).

## 4. Counting: bucket 6 and the count buffer [V][O]

- All volumes go to render bucket 6 (FUN_0049e280 = … 4 calls in the decomp, 4 jal words in the
  ELF: the two set-up branches, the tail, the draw). Buckets run 0..0x14 in order (`re-sky-reflect.md` 3): 6 comes
  after 3 (terrain) and 5 (opaque models) and before 8 (near water), 9-10 (the far target) and 0xd (translucents).
- **Set-up, each frame** (FUN_004a57e0 from FUN_0049b9a8, 565298-565317, DAT_00522b4c = 0: ELF 0, its stores the init
  and a setter at 0x4a8170 that nothing calls [receipt:22398fe5]): FUN_0049aa68(ctx, 0x6eeea0) binds and clears to 0
  a 512 x 448 CT32 target at FBP 0x180 (RAM: FRAME 0x80180, ZBUF 0x130000100 = the main Z, write-masked), then the
  draw state (0x6eed30: FRAME 0x80180, ZBUF 0x30000100), the default record {ALPHA 0x48 = Cs·As + Cd, TEST 0x51001 =
  Z GEQUAL with no Z write}, colour 0x80010101 and **COLCLAMP = 0: colours wrap mod 256**.
- **The volume passes** (FUN_004adbc8 570764-570782, ELF 0x4ae34c-0x4ae430): draw 1 with the default record; when
  near, record {0x10042: ALPHA 0x42 = Cd - Cs·As, TEST 0x31001 = Z ALWAYS} and draw 2 of the same list; then the
  default record again. Both records carry VU pass flag 0x100 (word0 >> 8); the call's three other pass slots point at
  0x54a5a0 (flags 3 = skipped), so one pass draws. The slot FUN_004ae6a8 skips a piece whose container word is 0.
- **The sign** (VU1): any pass with flag 0x100 (or 8) makes the batch run the facing loop 0x636-0x659 (0x620-0x62a),
  which writes (screen-space strip winding XOR the vertex's 0x4000 seed) into bit 0x4000 of each vertex; the CPU's
  seed (P / extruded, DAT_00522eec = 1) undoes the strip's alternate winding. The pass routine then (0x17f -> 0x544)
  turns the colour of every vertex with bit 0x4000 into 131074 - (65536 + c) (0x544-0x552): RGB 1 becomes 255.
  [D] Flat shading (prim 0x40, no IIP) takes the kicking vertex's colour, so a triangle facing one way adds 1, the other way 255 = -1 mod 256.
- **So** [D]: far = z-pass (front faces in front of the scene +1, back faces -1, no caps); near = draw 1 minus draw 2
  = minus the signed sum of the faces BEHIND the scene = z-fail ("Carmack's reverse") with both caps. Either way a
  pixel inside a volume ends at a non-zero count in R, G and B. Which facing gets +1 does not matter.

## 5. The darkening [V][M][O]

- FUN_00499f38 (at load, from FUN_00173a98) builds a display list (DAT_00522bac, 560700-560747): COLCLAMP = 1, the
  main target (0x6eeef0), then a full-screen textured sprite (PRIM 0x156) over 512 x 448 in 16 strips: TEST 0x31001,
  colour (0, 0, 0, 0x80), **TEX0 = the count buffer** (0x6eeed0 | TCC: TBP0 0x3000 = FBP 0x180, PSMCT24), TEXA =
  0x30 | 0x8000 (TA0 = DAT_00522b6c = 0x30 from FUN_0049a740, AEM on), ALPHA 0x44.
- FUN_004a7098 (each frame end, from FUN_0049bf50) appends a CALL of that list to bucket 6 (566031-566034), the only
  reader of DAT_00522bac (addrscan: 0x4a711c) [receipt:cf41b9cd].
- GS [D]: PSMCT24 + AEM gives a texel alpha 0 where the count is (0,0,0) and 0x30 elsewhere; modulated by the black
  vertex colour Cs = 0, As = 0x30; ALPHA 0x44 = Cd · (1 - As/128). **Inside a volume: Cd x 0.625; outside: untouched.**
  Overlapping volumes darken once. The shade is black: no colour, no per-map amount (BF1 `SunInfo ShadowColor` goes
  to FUN_00493120, an empty function).
- **[M] Live (PCSX2, BF1 NAB1, the VREST X-wing pose, `ps2/texa.py`):** the list's TEXA qword (RAM 0xdaf0d0, read back
  each time) poked to TA0 0x30 / 0x80 / 0x00 / 0x60 / 0x30, then the gate DAT_00522ed0 = 0. Over the band the 0x80
  frame paints black (3.49 % of the frame after excluding the HUD and the moving soldier; 5.2 % unfiltered), the luma
  ratio against the gate-0 frame (`tools/texa_measure.py`):
  | TA0 | predicted 1 - TA0/128 | measured |
  |---|---|---|
  | 0x30 (shipped) | 0.625 | 0.585, 0.586 |
  | 0x80 | 0 | 0.017 |
  | 0x60 | 0.25 | 0.229 |
  | 0x00 | 1 | 0.995 |
  | gate 0 | 1 | 1.000 |
  The displayed frame is 4-8 % under the prediction because later passes (the glow, the composite) act on the
  darkened values [H: which pass, not traced]. FXC's own pairs fit a multiply, not a subtraction: the ratio stays
  0.61-0.64 across off-luma bins 0-100 (`tools/onoff.py`). Sheet `ps2/xw_texa_sheet.png` (row order i0..i5).
- The black band lies on the ground; the X-wing's own hull and the soldier in front stay unshaded at this pose
  (z-equal near cap / no volume between them and the eye).

## 6. BF2 draws no shadow volume [V][M][O]

- The piece (B2 FUN_00592f08) gets vtable 0x6500d0; FUN_00592c38 calls slot +0xc like BF1; ELF word 0x6500dc =
  0x00593090 and 0x593090 is `addiu sp,sp,-32; jr ra; addiu sp,sp,32` - an empty function (BF1's slot is FUN_004ae6a8,
  the real draw). The rebuild still runs (FUN_00593068 -> FUN_005930a0 -> FUN_00592668).
- [M] RAM: BF2 HOT1 (two states) and GEO1 hold 26 pieces each, every one with vtable 0x6500d0 and the 3-instruction
  stub at its slot +0xc; BF1 NAB1 holds 50 with 0x4ae6a8 (`tools/b2pieces.py`).
- BF2 builds the darkening list's twin (FUN_00581018, TEXA from DAT_0060c2d0 = 0x30, 716153-716192) and stores it at
  DAT_0060c310, which nothing reads [receipt:02bc10f8]; `Effect("Shadow") Intensity` reaches DAT_007c5758 through
  FUN_0045f578 -> FUN_0057fd28, which nothing reads [receipt:3dc255e4]; no BF2 code builds the volume passes' record
  0x10042 or colour 0x80010101 [receipt:1d46a3d4]. The gate DAT_0060c66e and its one-view rule (`re-chunk-census.md`
  3) only switch the rebuild.
- So the faithful BF2 battle has no vehicle shadow. (Soldiers, droidekas and pickups have a separate `player_shadow`
  blob in both games - `RedShadowBlob` / `DropShadowRenderable`, B2 FUN_00239bb0 155383, B1 FUN_002013a0 113457 - not
  this card's; card SBLOB.)

## 7. What the port builds (spec for the build cards)

- **Data** (card VSHD, build): parse `shdw` (mungefs) and bake each vehicle's pieces {node name, positions, half-edges
  as {vertex, next, twin}} beside its model; BF1 sides only (BF2's are not drawn).
- **Draw** (card VSHD2): per BF1 vehicle with pieces, per frame: L_world = the map's `sun_dir`; len = factor x
  terrain hit fraction (flyer 500, hover / walker 150, the ray starts above; ELF constants, not PROVISIONAL);
  per piece in its node's frame: lit faces by n . L_local < 0, silhouette edges between lit and
  unlit, side quads extruded by len, caps = the unlit faces and their copy moved by len.
  Stencil after the opaque world and models, before water / far / translucent; then one full-screen pass: where the
  stencil is non-zero, colour x 0.625 **in the PS2's frame units** (the port's targets are `Rgba8UnormSrgb`, whose
  blend is linear: a GS-unit multiply is ~0.625^2.2 in linear, or a shader pass on the decoded value). No per-map
  key, black, never on BF2.
- **Named deviation (proposed):** z-fail with caps at every distance. The PS2's z-pass beyond 30 m + radius goes wrong
  when the eye is inside a long volume (a flyer's reaches 500 m); z-fail matches it wherever it is right.
- **Needs:** a stencil aspect (`Depth32Float` -> `Depth32FloatStencil8`, a device feature, or `Depth24PlusStencil8`,
  which moves depth precision and so goldens) - the build decides and declares.

## 8. Cards

- VSHD -> phase build (data: mungefs `shdw` + bake), acceptance: a mungefs test that parses all 404 disc chunks with the
  half-edge invariants of 1, and the baked pieces of the NAB1 X-wing.
- **VSHD2** (new, M1, 4): the stencil pass + darkening, deps VSHD; acceptance at the VREST pose against `ps2/` (the card's Acceptance).
- **SBLOB** (new, M1, 3, re): the soldier / droideka / pickup `player_shadow` blob, both games.

## 9. Checks run

`checks/fn.txt` (check 1 on 63 cited lines: each in the function named), `checks/words_writers.txt` (check 4: B1
0x522ec8-0x522f00, 0x522ed4 = 512.0, 0x522b44 = 30.0, 0x522b4c = 0, 0x522b6c = 0x30, 0x522ab0 = 30.0, vtable
0x559340, the record immediates 0x4ae360-0x4ae424; B2 vtable 0x6500d0, code 0x593090, 0x60c2d0 = 0x30; check 5:
writers of DAT_00522ed0 / ecc / ed4 / ed8 / edc / eec / ef8 / efc (0 each), b44 (2), b4c (1), b6c (1), bac (1), B2
0x60c310 (2), 0x7c5758 (2), 0x60c2d0 (1)), `checks/elfscans.txt` (widescan / addrscan of those addresses: stores only
where the decomp has them, plus the dead setter 0x4a8178), `checks/absent.txt` ([receipt:02bc10f8] [receipt:3dc255e4]
[receipt:1d46a3d4] [receipt:22398fe5] [receipt:1487da30] [receipt:d7704603] [receipt:3476da75] [receipt:cf41b9cd]).
Verifier (workflow `vshd-re-verify`, 6 agents, effort medium; `checks/verifier.json`): batch A (volume build, 8
claims), B (length / light / near / the slot's arguments, 7), C (count buffer, darkening, VU1, 8), M (the four
measurements re-run from the saved files, 4): 27 SUPPORTED. The six absence / twin claims (X1-X6), each read by two
separate verifiers with their own searches: X1, X3, X4, X5, X6 SUPPORTED by both; X2 PARTLY by both - the gate
DAT_00522ed0 does have an ELF store, the dead setter at 0x4ae850 (FXC's [receipt:af3dc61d]), so "no writer anywhere" was wrong for it;
the conclusion (it stays 1) holds and the text above now names the setter. No claim rests on one reader; the second
reader overturned nothing. Their extras folded in: the three skipped pass slots (4), the slot's container test (4),
XYOFFSET restored after the clear, BF2's flag-0x10000 setter FUN_00213ff0 (B2:127332, under the BF2 gate) feeds only
the rebuild + the empty slot.

## 10. Corrections to older docs

- `re-chunk-census.md` 0 / 3 / 6 and RE_LOG 2026-10-16 (FXC): "BF2: on while fewer than 2 players, darkness per map
  (`Effect("Shadow") Intensity`)" - BF2 rebuilds the volumes under that gate but never draws them (6 here), and the
  Intensity it stores is never read [receipt:3dc255e4]. BF1's "9.5 % of the frame, on / off 0.63" was measured on a mask of pixels darker
  by over 4 levels; the count != 0 region at that pose is 5.2 % of the frame and its ratio 0.585 (5).
- `re-chunk-census.md` 2's [H] "the darkening's colour and amount: SunInfo ShadowColor" - black, x 0.625, no key (5).

## Stayed [H]

- Which pass after bucket 6 lowers the displayed ratio from 0.625 to 0.585 (glow / composite).
- Whether any draw outside the four vehicle paths sets flag 0x10000 (a text search found none).
- BF1 split screen: FUN_004a8890 case 6 points each view's bucket 6 at FBP 0x180 when DAT_00522b4c == 0 - read, not run.
