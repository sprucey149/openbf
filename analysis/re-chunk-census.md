# OPUS RE (2026-10-16) - [O] UNAUDITED

# RE: the chunks mungefs does not parse - `shdw` (vehicle shadow volumes), `CLTH` (cloth), `bnd_` (the boundary) (card FXC)

Question (card FXC, HANDOFF HOLD 4): which LVL chunks the port skips may the PS2 draw? A census per chunk (maps, counts, what
it drives on the PS2) and one card per chunk worth building. `fx__` went to FXP (done) and `font` to BHUD (done); this pass
covers `shdw` (+ its `SHDI` / `SHDV` children), `CLTH` and `bnd_`.

Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`, both ELFs, both discs (every LVL under
the extracted `lvl/` trees), one PCSX2 knock-out (BF1 NAB1). Tags: [V] read (line checked with `re_check.py fn`), [D] derived,
[H] hypothesis, [M] measured on the PS2. Load-bearing claims carry [O].

Hops: disc census (chunk walker) -> the `modl` loaders' `shdw` branch (B1 FUN_00497150, B2 FUN_0057de00) -> the model draws'
shadow call (B1 FUN_00496248, B2 FUN_0057cf60) -> the shadow draw and its gate (B1 FUN_004ad760 / FUN_004adbc8 on
DAT_00522ed0; B2 FUN_00592c38 on DAT_0060c66e) -> the gate's writers (B2: the `Shadow` effect object FUN_0045f330 /
FUN_0045f428 / FUN_0045f3f0) -> the `fx__` Effect("Shadow") blocks on the disc -> a live knock-out of B1's gate. CLTH: the
BF2 level loader (FUN_00177058) -> FUN_001d9be0 -> the cloth classes and the soldier key `ClothODF`. bnd_: the level loader
-> the boundary object (card BNDY).

Tools (all under `engine/artifacts/lead_tmp/fxc/`): `tools/census.py` (every shdw / SHDI / CLTH / bnd_ chunk, TSV
`census.tsv`), `tools/effects.py <kind>` (every `Effect(...)` block of every `fx__`), `tools/bnd.py`, `ps2/knock.py` (the
knock-out), `checks.txt` / `checks_absent.txt` (the re_check runs), `claims.md` (the claims as the verifiers got them).

## 0. The answers

| chunk | on the disc | what the PS2 does with it | the port | card |
|---|---|---|---|---|
| `shdw` (`NAME INFO SHDV SHDI`) | inside `modl` only: BF1 240 chunks in the four side LVLs + NAB2_H (27 models: most vehicles, + rep_bldg_forwardcenter); BF2 164 in four side LVLs (11 models) | a **shadow volume** per model, rebuilt along a light vector every draw and drawn after the model: the ground under / beside a vehicle goes dark. BF1: always on. BF2: on while fewer than 2 players [H], darkness per map (`Effect("Shadow") Intensity`) | not parsed, no shadows at all | **VSHD** (new, M1, 4) |
| `CLTH` (`INFO NAME PRNT XFRM DATA COLL`) | BF2 only: 58 chunks in 10 files (INGAME, 8 side LVLs, NAB2) | **cloth**: a separate textured mesh hung on a bone with collision capsules, named by the class key `ClothODF`: Hoth snowtroopers' skirts, the ep3 clone sniper / heavy / commander capes, hero capes and robes, the CTF flags | not parsed: those soldiers are drawn without it | **CLOTH** (new, M1, 5) |
| `bnd_` (config) | 18 BF1 map LVLs and 25 BF2: `Boundary { Path("<name>") ... }`, 0-4 paths (8 BF2 maps, UTA1 and MYG1 among them, list none; BF1 YAV1 / YAV2 have no chunk) | names the paths whose outline is the battlefield's boundary (the level loader hands the chunk to the boundary object) | not read; no boundary | note on **BNDY** (exists, M3, 4) |

## 1. The census [D][O]

`tools/census.py` walks every LVL (descending `ucfb`, `lvl_`, hashed sub-level containers and `modl`) and lists every chunk
tagged `shdw`, `SHDI`, `CLTH` or `bnd_`:

| game | tag | chunks | files | bytes |
|---|---|---|---|---|
| BF1 | shdw | 240 | 5 (SIDE/ALL 54, CIS 116, IMP 214, REP 18 - counted with their LOWD twins - and NAB/NAB2_H 2) | 452,584 |
| BF2 | shdw | 164 | 4 (SIDE/ALL, CIS, IMP, REP) | 261,392 |
| BF2 | CLTH | 58 | 10 | 508,348 |
| BF1 | bnd_ | 18 | 18 map LVLs | 1,152 |
| BF2 | bnd_ | 25 | 25 map LVLs | 1,648 |

- Every `shdw` is a child of a `modl` (the model and its `...LOWD` twin each carry one) with children `NAME INFO SHDV SHDI`.
  `SHDI` never appears outside a `shdw`. BF1 has no `CLTH` at all. The raw byte count of the string `shdw` is higher on six BF2
  space maps: those hits are region names in `PROP` strings (`rep_shdw1`, `imp_shdw1`), not chunks.
- **Models with a volume** (LOWD twins merged): BF1 (27) all_fly_kwing, all_fly_snowspeeder, all_fly_xwing, all_fly_ywing,
  all_hover_hovernaut, all_hover_rebel_attack_speeder, cis_fly_droidfighter, cis_fly_geofighter, cis_fly_maf, cis_hover_aat,
  cis_hover_mtt, cis_hover_stap, cis_tread_hailfire, cis_walk_spider, imp_fly_tiefighter, imp_fly_trooptrans,
  imp_hover_fightertank, imp_hover_speederbike, imp_walk_atat, imp_walk_atst (+ _jungle, _snow), rep_bldg_forwardcenter,
  rep_fly_vtrans, rep_hover_fightertank, rep_hover_gian_speeder, rep_hover_speederbike. BF2 (11) all_fly_ywing,
  cis_fly_droidgunship, cis_fly_maf, cis_walk_spider, imp_fly_tiefighter, imp_fly_tieinterceptor, imp_fly_trooptrans,
  imp_walk_atst (+ _jungle, _snow), rep_bldg_forwardcenter. No BF2 hover tank, AAT, AT-AT or AT-TE has one; in BF1 the TIE
  bomber, the Jedi starfighter, the AT-TE and the skiff have none either (verifier C4); no soldier model in either game has one.
- On the showcase battles the port plays: BF1 NAB1 (AAT, STAP, the Republic fighter tank; the GCW era's AT-ST, speeder bike,
  X-wing), BF1 YAV2 (spider walker, AT-ST, fighter tank), BF2 HOT1 (the AT-ST and AT-ST snow pads; its AT-ATs carry none),
  BF2 MYG1 (one pad names imp_walk_atst, baked manifest line 8705). BF2 UTA1's vehicle classes have none [D, from the baked
  manifests' class names; which era's pads a battle uses was not checked].

## 2. `shdw` on BF1 [V][O]

- **Load.** The `modl` loader FUN_00497150, on a `shdw` child (0x77646873, B1 558126), creates once a shadow container at
  model +0xb8 (FUN_004ad6c0, 558129) and hands the chunk to FUN_004ae478 (558137), which parses it into a piece
  (FUN_004acb20: `SHDI` 569481, `SHDV` 569524) linked on the container's list; the piece's vtable is DAT_00559340
  (FUN_004ae518, 570835).
- **Draw call.** The model draw FUN_00496248 calls FUN_004ad760(container, matrix, ..., near) when model +0xb8 != 0 and
  `(flags & 0x230000) == 0x10000` (557404-557409); `near` = camera distance < DAT_00522ab0 (30.0) + the model's radius.
  The same model also takes the vehicle light set (`FUN_00493cf0(..., model+0xb8 != 0)`, 557187; card VAMB).
- **Gate.** FUN_004ad760 draws nothing unless DAT_00522ed0 != 0 (570064). Per piece: FUN_004ae680 stores the matrix and
  the vector FUN_0049b8a8 returns [H: the sun direction] and rebuilds the volume (FUN_004ae700 -> FUN_004ad180, run when
  the container - piece+0x28 - has its owner model in word 0, which the loader always writes; verifier S5), then the
  vtable slot +0xc (word 0x004ae6a8 at 0x55934c) -> FUN_004ae6a8 -> FUN_004adbc8, the draw, which also returns when
  DAT_00522ed0 == 0 (570393).
- **The gate is always on.** DAT_00522ed0's ELF word is 1 (.data). Its only store in the ELF is a three-instruction
  setter at 0x4ae850 (`lui v0,0x52; jr ra; sw a0,0x2ed0(v0)` in the delay slot, not in the decomp export) that has no `jal`
  word, no pointer word and no lui/addiu pointer formed to it [receipt:af3dc61d] (widescan bf1 0x522ed0: two loads
  0x4ad784 / 0x4adc28, one store 0x4ae858; addrscan 0 materialisations). So every BF1 battle draws the volumes.
- **[M] Knock-out (PCSX2, BF1 NAB1, a parked X-wing in view, `ps2/knock.py`).** The gate written 1, 0, 1, 0 (RAM read
  back each time), one grab each: the pixels darker in both "on" frames than in both "off" frames cover 9.5 % of the frame,
  a band of ground in front of the X-wing (`ps2/xw_dark_overlay.png`), darkened by a median 20 grey levels, on / off
  brightness 0.63. The ground under the left wing reads 67.4 / 67.4 on against 70.6 / 70.5 off; the sky moves under 0.5.
- [H] The darkening's colour and amount: BF1's `SunInfo` third key is `ShadowColor` (RE_LOG, model-lighting audit); its
  reader was not traced. The build card measures the shade on the PS2 frames.

## 3. `shdw` on BF2 [V][O]

- **Load and draw** are BF1's twin: FUN_0057de00 on `shdw` (713687) -> FUN_00592b98 / FUN_00592e68 -> FUN_00592050
  (`SHDI` 725420, `SHDV` 725435); the model draw FUN_0057cf60 calls FUN_00592c38 under the same +0xb8 / 0x230000 / 30 m
  rules (712961-712966) - one difference, found by both twin readers: BF2's distance is the view depth MINUS the camera
  object's float at +0x130 (FUN_00582c60), BF1's is the plain view depth; FUN_00592c38 draws nothing unless the byte
  DAT_0060c66e != 0 (725805).
- **The gate.** DAT_0060c66e (ELF default 0) is written only by FUN_0045f540 (522339), which also copies it to
  DAT_0060c714. Writers of the value:
  - FUN_0045f330, the constructor of the **Shadow effect object** (registered under FNV("shadow") 0x3a43c561): value 1,
    intensity (obj+0x28) 0.5, obj+0x24 = 0 (522236-522250). It is built once (FUN_0045f2b0 -> DAT_00601330) from the effect
    start-up FUN_00450cf8 (512878), which the level start FUN_00174b28 calls (16910): every battle starts with the gate on.
  - FUN_0045f428 (vtable word at 0x63a3e8), the parser of a map's `Effect("Shadow")` block: `Intensity` (0x8563e50a) ->
    obj+0x28 clamped to [0, 1]; `Enable` (0xaf8bb8ce) -> the gate; key 0x1ed46057 -> obj+0x24 (a bool, name not cracked);
    at the end of the block it writes the gate = `FUN_00563f78() < 2` (522301), so the block's own `Enable` does not
    survive the parse.
  - FUN_0045f3f0 (vtable word at 0x63a3dc): the gate = `FUN_00563f78() < 2` (522275). FUN_00563f78 returns DAT_00746014
    [H: the number of local players / views - shadows off in split screen].
- **The disc** (`tools/effects.py shadow`): `Effect("Shadow")` with `Enable(1)`, `0x1ed46057(0)` on BF2 FEL1 (Intensity 0.5),
  TAT2 (0.35) and **UTA1 (0.2)**; every other BF2 map keeps the constructor's 0.5; BF1 has no such block. `Effect
  ("WorldShadowMap")` is a different effect (a projected canopy / cloud texture: BF2 END1 on with `end_canopy`; BF1 END1 /
  HOT1 and BF2 HOT1 off) - not this card's.
- [H] Where obj+0x28 (Intensity) is read was not traced: the build card's RE opens FUN_00592c38's draw for it.
- **Correction (card VSHD, 2026-10-16, `re-vehicle-shadow.md` 6, 10):** BF2 never DRAWS the volume - the piece's draw slot
  (vtable 0x6500d0 +0xc = 0x593090) is an empty function in the ELF and in the RAM of three BF2 battles; this gate only
  switches the rebuild. The Intensity goes to DAT_007c5758, which nothing reads [receipt:3dc255e4]. BF1's darkening (2's
  [H]) is black, x 0.625 inside any volume, with no per-map key; the knock-out's count != 0 region is 5.2 % of the frame
  (ratio 0.585), not the 9.5 % / 0.63 of the darker-by-4-levels mask.
- `ScriptCB_DisableFlyerShadows` (ten BF2 space-map scripts call it) writes … FUN_00174b28 sets 1,
  16761), and nothing reads DAT_005f908c [receipt:fccf2693] (widescan bf2 0x5f908c: the two stores only).

## 4. `CLTH` - BF2 cloth [V][D][O]

- **Load.** The BF2 level loader FUN_00177058 hands `CLTH` (0x48544c43, 18345) to FUN_001d9be0 (18346): a 0x90-byte
  object parsed by FUN_001d9ca8 (its `COLL` child, 0x4c4c4f43, into 0x60-byte entries) and registered by name in the table
  at 0x686ba8 (86537-86562). BF1 has no CLTH loader [receipt:c3674481] (no compare in B1, no lui/ori immediate of the tag
  in the BF1 ELF: `tagscan bf1 CLTH` = 0).
- **The chunk** (IMP.LVL `imp_inf_snowtrooper_skirt`): `INFO` the texture (`imp_inf_snowtrooper`), `NAME`, `PRNT` the bone it
  hangs on (`bone_root`), `XFRM`, `DATA` (the texture name again, then 54 vertices and the mesh), `COLL` (4 collision shapes
  on bones: `bone_l_thigh`, ...). The cloth is a mesh of its own: the soldier's `modl` does not carry it.
- **Who wears it.** 32 of the 42 distinct CLTH names are classes with BASE `cloth` (first key: the owner's geometry) by the
  census verifier's own walk (C7); the other 10 are chunks whose name matched none of the cloth classes it listed (e.g. the
  `*_wookie_dreads1..4`, `CIS_CountDooku_Cape_Low1`, and `rep_inf_ep3cloakedanakin_skirt`, which ClothODF names), and
  NAB2's two lamp-post cloth classes matched none of the CLTH chunks [D, one walk each by the lead and the verifier]. A
  soldier / item / prop names its cloth with `ClothODF` (0xf4d25eda = FNV("ClothODF")): imp_inf_rifleman_snow /
  rocketeer_snow / sniper_snow -> imp_inf_snowtrooper_skirt; rep_inf_ep3_sniper (+ _felucia) -> rep_inf_ep3sniper_cape;
  rep_inf_ep3_rocketeer -> rep_inf_ep3heavytrooper_cape; rep_inf_ep2_rocketeer (+ _chaingun) -> rep_inf_arctrooper_cape;
  rep_inf_ep3_officer -> rep_inf_clonecommander_cape; gar_inf_naboo_queen (3); the heroes Vader, Emperor, Boba Fett, Dooku,
  Maul, Anakin (2), Obi-Wan, Mace (2), Yoda, Ki-Adi-Mundi, Leia, Chewbacca (2), jed_sith_01; the CTF flags com_item_flag_*
  (10 cloth classes); NAB2's lamp post (2, no chunk). The Wookiees' and Geonosians' `DREADS` / `WINGS` keys (0xe8888b42 +
  0x94272331) are a different attachment, not ClothODF [H: the ALL / WOK `*_wookie_dreads1..4` CLTH chunks have no cloth
  class naming them].
- **On the showcase battles the port plays:** BF2 HOT1 (GCW conquest): three of the six Imperial classes - the rifleman
  (the most common unit), rocketeer and sniper - wear the snowtrooper skirt; Vader as the Imperial hero. The PS2 frame
  `pcsx2/ref/bf2_hot1/cp1_cp6_postA_spawn.png` shows the player's own snowtrooper with the white skirt to the knees, centre
  screen; the port's `lead_tmp/two-tier-soldier/units_after/bf2_imp_inf_snowtrooper.png` shows bare brown hips. BF2 UTA1 /
  MYG1's GCW conquest fields no ClothODF class (Clone Wars clones carry the capes).
- [H] The cloth simulation (step, constants, the class keys 0x94e4e309 / 0x13901a72 / 0xc1133964 / 0xb88c0d43 / 0xafe34731)
  was not read: the build card's RE.
- **Correction (card CLOTH, 2026-10-16, `re-cloth.md` 2):** the game does not read `ClothODF` (its hash is nowhere in the
  BF2 ELF); a cloth is attached by the model's `gmod` INFO list (B2 FUN_001c59e8) plus a `cloth` class of the same name.
  The ClothODF lists above are the munger's packing references; for the showcase units (the HOT1 snow classes, Vader)
  they name the same cloths as the gmod lists. The class keys above are Priority / WindSpeed / Dampening / Drag / Transparent (`re-cloth.md` 3).

## 5. `bnd_` - the boundary's paths [V][D]

- B2's level loader hands `bnd_` (0x5f646e62, 18380) to FUN_0034c3b0(DAT_005feca8, chunk): the boundary object card BNDY
  already names (its point test and timer are read in `re-announcer.md` 2.4).
- The chunk (`tools/bnd.py`) is one config statement `Boundary` (DATA 0x54eacca7 = FNV("boundary")) whose scope lists one
  statement 0x84874d36 = FNV("path") per path, its argument the path name's hash. BF1: one path on each of the 18 maps
  (`boundary` on BES1, NAB2(_H), RHN1, RHN2, TAT1(H); `boundary1` on the other 11; YAV1 / YAV2 have no `bnd_` chunk
  [receipt:ae8beb67], the same scan finding 3 in the TAT LVLs). BF2:
  FEL1 4, HOT1 3 (`boundary1..3`), SPA2 3, KAS2 / SPA7 / SPA9 2, END1 / GEO1 / KAM1 / POL1 / SPA1 / SPA3 / SPA4 / SPA6 /
  SPA8 / TAT2 / YAV1 1, and **an empty scope on COR1, DAG1, DEA1, MUS1, MYG1, NAB2, TAN1, UTA1** (verifier C8, recounted).
  With no shapes the PS2 is never "outside" (BNDY, from re-announcer 2.4) [D]: two BF2 showcase maps and BF1 YAV2 have no
  boundary from `bnd_` [H: unless the boundary object also takes paths from elsewhere - BNDY's RE settles it]. The paths
  themselves are `path` chunks the port already parses (`mungefs::path`, the kind-2 "boundary" paths). Folded into BNDY as
  a note; no new card.

## 6. Cards

- **VSHD** (M1, value 4, phase re): vehicle shadow volumes. BF1 always; BF2 while one player, darkness per map
  (UTA1 0.2 / TAT2 0.35 / FEL1 0.5 / default 0.5). Acceptance: the knock-out's PS2 pair (NAB1 X-wing) against the port at the
  same pose - the shadow band's area and on/off brightness ratio within a stated tolerance of the PS2's 9.5 % / 0.63.
- **CLOTH** (M1, value 5, phase re): BF2 cloth, first the snowtrooper skirt on HOT1. Acceptance: the port's
  imp_inf_snowtrooper draws the skirt (a pixel test on the unit still: white covers the hip box) and a PS2 | port pair of a
  Hoth snowtrooper.
- bnd_: note on BNDY.

## 7. Checks run

`checks.txt` (check 1 fn on every cited line, check 4 words: B1 0x522ed0 = 1, 0x55934c = 0x004ae6a8, 0x522ab0 = 30.0 /
0x522ab4 = 35.0, B2 0x60c66e = 0, 0x63a3dc = 0x0045f3f0, 0x63a3e8 = 0x0045f428, 0x60c21c = 30.0; check 5 writers of
DAT_00522ed0 (0), DAT_0060c66e (1), DAT_005f908c (2), DAT_00601330 (2); callers). `checks_absent.txt` (receipts af3dc61d,
c3674481, fccf2693, 48e1eebf, d7796afb; tagscan of CLTH / shdw in both ELFs; widescan of 0x522ed0 and 0x5f908c).
Verifier (workflow `fxc-re-verify`, 6 agents, effort medium): Set A (BF1 shadow, 8 claims) 7 SUPPORTED / 1 PARTLY (S5,
the rebuild's condition, folded in); Set B (BF2 shadow, 9) 9 SUPPORTED; Set X (6 absence / twin claims, each read by two
separate verifiers) X2, X3, X5, X6 SUPPORTED by both, X1 SUPPORTED + PARTLY (the setter is three instructions, not two:
wording, folded in; the absence held under both readers' own searches - jal, j, data words, lui pairs, addrscan), X4 PARTLY
by both (BF2's near distance subtracts camera +0x130: folded in; the gate test and the 30 m threshold are the twins);
Set C (census, 8) 5 SUPPORTED, 2 PARTLY (C4 "every vehicle", C7 the cloth-class mapping: folded in), 1 NOT SUPPORTED (C8,
the bnd_ details: recounted and rewritten in 5); Set M (the knock-out, 3) 3 reproduced, the darkening 8-10 times the
same-state frame noise in the mask. The second reader overturned nothing; no claim rests on one reader alone.
