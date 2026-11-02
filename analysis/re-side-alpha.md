# OPUS RE (loop lead, 2026-09-17, lane 1) - [O] UNAUDITED: no Fable pass

**What does the disc do with a SIDE model's alpha?** — queue item **SKA**'s `[RE]` half, i.e. design
**D6**'s unanswered half and the reason queue AT (`a55688f`) deliberately did NOT extend the disc's world
cutout test to skinned draws. Method: `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` §5 — the four
self-checks first, then ONE Opus verifier agent given the citations alone. Scratch scripts, census JSON and
raw measurements: `engine/artifacts/lead_tmp/re/side-alpha/`. Corpora: `analysis/SWBFRONT.ELF-decomp.c`
(BF1), `analysis/SWBFRNT2.ELF-decomp.c` (BF2); ELF words and instruction bytes read with that directory's
`elfrd.py` / `findword.py` / `findstr.py` (both ELFs' single PT_LOAD maps `off = va - 0x15f000`, i.e.
`va - 0x160000 + 0x1000`). Tags: [V] read at the line / measured, [D] derived, [H] guess; every
load-bearing claim also carries [O] — Opus-only, unaudited.

## 0. Answer

**The disc does not alpha-test a soldier at all, and it blends exactly one segment per game.**

1. **[V][O] No side SOLDIER or WEAPON segment on either disc carries the cutout bit 0x02.** Measured over
   every `modl` in all 10 BF1 + 18 BF2 SIDE LVLs (2,581 segment rows; 522 / 852 distinct segments), reduced
   to the models the port actually bakes: BF1 21 soldiers / 39 distinct segments and 13 weapons / 16
   segments, BF2 31 soldiers / 58 segments and 16 weapons / 20 segments — **0x02 count: zero, in all four
   sets** (§1). So the composition AT ported for world cutouts (`MASK_CUTOFF` = 0.753 = disc alpha ≥ 193)
   has no soldier to apply to, and neither does any other alpha test.
2. **[V][O] Exactly ONE segment per game carries the transparent bit 0x04, and it is the same segment:**
   `all_inf_wookie`'s fur body — BF1 segment 2, word `0x5`, 261 vertices, texture `wookie`; BF2 segment 1,
   word `0x10005`, 288 vertices, texture `all_inf_wookie`. Both games draw it BLENDED (GS `ALPHA 0x44` =
   `Cs·As + Cd·(1−As)`, `TEST 0x51001` = Z-tested with **no Z write**), with `As` = the sheet's alpha ×
   the draw colour's alpha, because TEX0 carries TCC=1/TFX=MODULATE for every texture in both games
   (`re-material-flags.md` §0.5 [V]).
3. **[V][O] Every other soldier segment is drawn OPAQUE with the sheet's alpha IGNORED** — `TEST 0x50000`
   (ATE=0, Z-test GEQUAL) and no blend. BF1 words: `0x1`×34, `0x31`×4 (bit 0x20 = the Specular class, which
   enables neither the test nor the blend, §4), `0x5`×1. BF2 words: `0x10001`×43, `0x10089`×12,
   `0x10101`×1, `0x10010`×1, `0x10005`×1.
4. **[V][O] The GS state of a skinned draw is decided by the segment's own on-disc `MTRL` word and by
   nothing else.** In both games the bone palette reaches the per-segment draw as two arguments that are
   referenced twice each and consumed by a single DMA call — BF2 `FUN_004975c0` (line 724020), BF1
   `FUN_0039bca0` (line 594499). No ALPHA word, TEST word, PRIM word or draw bucket reads them (§3, §4).
   **There is no skinned-specific material path in either ELF.**
5. **[V][O] The soldier's draw colour is not a tint.** BF2's soldier draw writes R = G = B = a clamped
   brightness scalar × 255 and alpha = (the draw's float argument) × 255 (§5); the scene's visible-object
   loop passes the literal `1.0f`, so a normal soldier's colour alpha is `0xff` and only the distance-fade
   path lowers it — which is exactly what makes a plain `0x10001` segment translucent at range on the disc.
   **Corrected 2026-09-23 (cleanse):** a possibly-rescaled float (§7); the verdict does not
   change.
6. **Consequence for the port, measured [V]:** the shipped `fs_mask_skinned` tests EVERY skinned segment at
   `SKINNED_MASK_CUTOFF` = 0.5. It is inert on the 31/32 (BF1) and 40/41 (BF2) side textures the bake
   force-opaques, and on the one sheet per game that keeps its disc alpha it deletes **93.44 %** of
   `all_inf_wookie`'s 65,536 texels (BF2, mean alpha 30.7) and **8.46 %** of `wookie`'s (BF1, mean 233.6).
   Both committed unit goldens show it: `engine/goldens/units/bf2_all_inf_wookie.png` is a cloud of fur
   fragments around an intact weapon (viewed 2026-09-17). The disc draws that body opaque in two of its
   three segments and blends the third. **The port's error is in KIND, not in threshold** — queue SKA's
   build half.

## 1. The data — (a) do side `MTRL` words carry 0x02/0x04 at all?

Census: `engine/artifacts/texture_census/mtrl_census.py` (unchanged, the session-12 tool) over both discs'
SIDE LVLs → `lead_tmp/re/side-alpha/side_rows.json`; reduced by `census_a.py` (raw) and `census_b.py`
(deduplicated: one row per model+segment, repeats across `ALL.LVL`/`ALLSHELL.LVL` checked identical —
**45/103 duplicate rows dropped, 0 conflicting**). Outputs are the `.out` files beside them.

**Instrument checked before use** [V]: the parser reproduces the two `MTRL`/`RTYP` vectors pinned in
`mungefs/src/model.rs:436` and `:455` from the disc bytes — BF1 `kam_prop_crate` seg 0 = `0x410` /
`"Scroll"` (12-byte MTRL), BF2 = `0x10089` / `0` (25-byte MTRL).

| set | models | distinct segments | 0x02 | 0x04 | textures named | named only by unflagged segments |
|---|---|---|---|---|---|---|
| BF1 baked soldiers | 21 | 39 | **0** | 1 | 32 | 31 |
| BF1 baked weapons | 13 | 16 | **0** | **0** | 13 | 13 |
| BF2 baked soldiers | 31 | 58 | **0** | 1 | 41 | 40 |
| BF2 baked weapons | 16 | 20 | **0** | **0** | 16 | 16 |

The bits are NOT unused in side data — they are unused on the models the port draws skinned [V]:
- 0x02 appears on 7 BF1 / 9 BF2 side models, and they are all the same thing: the `*_weap_inf_fusioncutter`
  family (its beam) plus `rep_walk_atte`.
- 0x04 appears on 25 BF1 / 39 BF2 side models: flyers and gunships (`all_fly_xwing`, `rep_la_at_gunship`,
  `cis_fly_tridroidfighter`…), hero units (`all_inf_luke`, `all_inf_chewbacca`, `rep_inf_ep3anakin`,
  `hot_inf_wampa`, `wok_inf_wookiewarrior`) and `all_inf_wookie`.
  **So P7 (vehicles) and any hero unit will need the per-segment verdict this entry builds** [D][O].

Texture side, measured on the shipped bake (`side_tex_alpha.py`, PS2 base tier, `.dtl` detail sheets
excluded — they are a separate layer and are not what `base.a` samples):

| sheet | size | alpha | mean | discarded at 0.5 | at 193 |
|---|---|---|---|---|---|
| `side_bf1/textures/wookie.ktx2` | 256² | 0..255 | 233.6 | **8.46 %** | 8.87 % |
| `side_bf2/textures/all_inf_wookie.ktx2` | 256² | 0..255 | 30.7 | **93.44 %** | 98.73 % |
| every other base sheet (45 BF1, 55 BF2) | — | 255 everywhere | 255 | 0 | 0 |

That reproduces the AT landing's two numbers exactly from the same files (4,299 and 835 of 65,536 kept =
6.56 % and 1.27 %).

## 2. The soldier's caller chain in BF2 — the hops

Every citation names its enclosing function (§5 self-check 1); decomp = `analysis/SWBFRNT2.ELF-decomp.c`.

1. **[V][O] The engine has an entity class label `soldier`** — the ASCII string at VA `0x0060fb78`, read
   from `extracted/bf2/SWBFRNT2.ELF` (file offset `0x4b0b78`), in the same table as `door` at `0x60fb68`
   (the class-label factory `re-bf2-doors.md` §2 hop 1 walked). Registered in `FUN_00173d10` at lines
   16390–16392: `FUN_0059d428(0x12b0)` (a 0x12b0-byte class object), *(decompiler detail omitted)*,
   *(decompiler detail omitted)*.
2. **[V][O] `FUN_00250488` is that class's constructor** and installs the class vtable at class+0x1c:
   *(decompiler detail omitted)* (line 169331). `0x61e8d8` sits between the two
   `EntitySoldier` / `EntitySoldierClass` RTTI strings at VA `0x0061e618` and `0x0061ece0` (read from the
   ELF), which bracket this class's structures.
   **Corrected 2026-09-23 (cleanse):** 169332 (§7).
3. **[V][O] Class vtable slot +0x10 is the instance factory:** the ELF word at VA `0x0061e8e8` is
   `0x00255558`, and `FUN_00255558` calls `FUN_00234a68` (line 172188) — the instance constructor, which
   installs the render base's vtable at `this+0xd0`: *(decompiler detail omitted)* (line 149405).
4. **[V][O] That vtable's slot +0x54 is the draw:** the ELF word at VA `0x0061eb0c` is `0x00255db8`, and
   `FUN_00255db8` is a `this`-adjusting thunk — *(decompiler detail omitted)* (line 172760).
   (Its neighbour at `0x61eb00` is the matching destructor thunk into `FUN_00235b48`, which installs the
   same vtable at line 150066 — the pair that confirms the base's identity.)
5. **[V][O] `FUN_00239bb0` is the soldier's per-frame draw** and it calls the `RedModel` vtable slot +0xc
   twice — lines 155280 and 155298 — passing a **bone palette** as argument 3 (*(decompiler detail omitted)*) and a colour
   buffer as argument 4 (*(decompiler detail omitted)*). This is the function `re-material-flags.md` §1's caller table
   listed as "155279/155297 `FUN_00239bb0` (\"player_shadow\" refs)": it is the SOLDIER's draw, and the
   table's label is the one thing in that row this RE corrects.
6. **[V][O] `RedModel`'s slot +0xc is `FUN_0057cf60`** — the ELF word at VA `0x0064febc` is `0x0057cf60`,
   with the RTTI string `8RedModel` at VA `0x0064fec8` (both read from the ELF, self-check 4).

## 3. BF2 — the per-segment rule, and where the palette goes

`FUN_0057cf60`'s segment loop (lines 712840–712956) has three branches and **all three call
`FUN_0058ed70`** [V][O]:

| branch | condition | call | palette |
|---|---|---|---|
| plain | *(decompiler detail omitted)*, `seg+0x84 == 0` | 712870 *(decompiler detail omitted)* | none |
| posed rigid | *(decompiler detail omitted)*, `seg+0x84 == 0` | 712920, same but *(decompiler detail omitted)* and the segment's own matrix | none |
| **softskin** | `seg+0x84 != 0` | 712952 *(decompiler detail omitted)* | *(decompiler detail omitted)* + count |

**Correction to `re-material-flags.md` §1** [V][O]: that section labels 712870/712920/712952
"rigid / skinned / bone-attached". Read at the lines, 712920 passes **0 and 0** for the palette arguments —
it is the POSED-RIGID branch (a hard-skin `BNAM` segment or a rigid segment of an animated model, drawn
with its bone's matrix, `re-softskin.md` §6) — and the branch that carries a matrix palette is 712952, the
one guarded by `seg+0x84 != 0`, whose three gathers `FUN_0057f158` / `FUN_0057e780` / `FUN_0057e8e8` are
exactly the ones `re-softskin.md` §1 names as the softskin matrix gathers. The labels for the second and
third branch are swapped in that section; the claim it rests on (all three reach `FUN_0058ed70`) is right.

**The rule inside `FUN_0058ed70` (lines 723257–724038), re-read at the lines** [V][O]:
- *(decompiler detail omitted)* → return without drawing (723395).
- translucent *(decompiler detail omitted)* = *(decompiler detail omitted)* **OR** `seg+0x18 & 4` **OR** *(decompiler detail omitted)* (723411);
  *(decompiler detail omitted)* is `seg+0x18`, the `MTRL` u32 stored verbatim by `FUN_005916b0` (724860).
- translucent → GS `ALPHA 0x44` (723453), `0x64` with bit 0x08 (723456), `0x48` with bit 0x40 or the force
  flag (723461); `TEST 0x51001` (723465/723474).
- not translucent → `TEST 0x50000` (723477), or `0x5000b` if `seg+0x18 & 2` (723478–723479), whose AREF is
  `DAT_0060c64c << 4`; the bytes at VA `0x0060c64c` are `60 00 00 00`, so AREF = **0x60** = disc alpha ≥ 193
  after the `pal_` CLUT scale (`re-hud-icons.md` citation 3) — AT's number, re-read here.
- **From the instruction bytes, not the decompiled literals** (self-check 4): `0058efa4: lui r23,0x0005`
  (the 0x50000 base), `0058f028` and `0058f038: ori r23,r23,0x1001` (the two 0x51001 sites),
  `0058f03c: ori r3,r23,0x000b` (0x5000b), and the three ALPHA values `0058f004: addiu r30,0x0048`,
  `0058f010: addiu r30,0x0064`, `0058f01c: addiu r30,0x0044`.

**ABSENCE, searched not read** (§5 self-check 2) [V][O]:
- `FUN_0058ed70` has exactly **3** call sites in the whole corpus (712870, 712920, 712952 — `grep -c` = 4
  including its own definition line) and its address `0x0058ed70` appears in **0** data words of the ELF
  image (`findword.py`), so no vtable or pointer table reaches it. There is no fourth caller and no
  skinned-only twin.
- Inside it, *(decompiler detail omitted)* and *(decompiler detail omitted)* are referenced **2× each**: the copies *(decompiler detail omitted)* (723383–723384) and the single use *(decompiler detail omitted)* (724019–724020), which is past every state-word
  decision. **A skinned draw and a rigid draw with the same `MTRL` word get byte-identical GS state.**
- The alpha-test word `0x5000b` occurs **4×** in the whole BF2 corpus: 723479 and 723630, both inside
  `FUN_0058ed70`, and 420304/420308 inside `FUN_003c3868`, which does nothing but assign a static block of
  `DAT_006a9a**` state words — not a model path.

## 4. BF1 — the twin, opened (§5 self-check 3)

- **[V][O] The same class label:** `soldier` at VA `0x005271a8` (`extracted/bf1/SWBFRONT.ELF`), registered
  at lines 15579–15581 (`…
  `FUN_0020d238` installs `&DAT_00532b20` at +0x1c (line 121778); the ELF word at VA `0x00532b30` (class
  vtable +0x10) is `0x002109e8`, and `FUN_002109e8` allocates `0xcc0` bytes and calls the instance
  constructor `FUN_001ff1e8` (line 123690).
  **Corrected 2026-09-23 (cleanse):** 123696/123697 (§7).
- **[V][O] BF1's `RedModel` is the same class, at the same vtable slot:** the vtable base is
  **`0x00558fe8`** — the address two constructors store at object+0xc0, `FUN_00496068` line 557008 and
  `FUN_004960e8` line 557038, exactly as BF2's store `&DAT_0064feb0` there (712517/712549) — with RTTI
  `8RedModel` at `0x00559028`. The ELF word at `0x00558ff4` is `0x00496248`, i.e. slot **+0xc**, the same
  slot as BF2's. **Corrected 2026-09-17 after the verifier pass** (§7): this section first read the base as
  `0x00558fe0` and the slot as +0x14, because BF1's vtable is preceded by two zero words that BF2's is not;
  the constructor stores settle it and the two games' model vtables are structurally identical —
  `[0,0,A,B,C,D,0]` with `A/B/C/D` = `0x497a50/0x4960e8/0x496248/0x496240` (BF1) against
  `0x57e6a8/0x57cdf0/0x57cf60/0x57cf58` (BF2). `FUN_00496248` is the function `re-softskin.md` §1 already
  names as BF1's "model draw (per segment, matrix gather)", and its signature
  `(model, matrix, pose, colour*, flags, attach)` matches `FUN_0057cf60` argument for argument.
- **[V][O] Three branches again**, calling the SEGMENT object's slot +0xc with BF2's 7-argument shape:
  557311 `(seg,matrix,0,0,flags,colour,attach)`, 557362 (posed), and **557396
  *(decompiler detail omitted)*** — the palette branch.
- **[V][O] BF1 picks a segment CLASS from the `MTRL` word instead of branching inside one draw**
  (`FUN_004aa9f8`, lines 568106–568159): bit 0x20 → vtable `0x5594a0`; else 0x200 → `0x55e310`; else 0x400
  → `0x559408`; else 0x800 or 0x4000 → `0x55e278`; else `0x55e310`. Slot +0xc of each, read from the ELF:
  `0x55e310`+0xc = `0x004cf6f0` (Normal), `0x5594a0`+0xc = `0x004b2310` (Specular), `0x559408`+0xc =
  `0x004b0c48` (Scroll), `0x55e278`+0xc = `0x004cf0a8` (Blur).
- **[V][O] The cutout/transparent bits become a MODE at load** (`FUN_004abde0`, lines 568939–568954):
  bit 0x02 → mode **1**; else bit 0x04 → mode **2**; else bit 0x08 → mode **3**; else **0**. Stored at
  `seg+0x2c`; the raw word goes to `seg+0x38` (568927).
- **[V][O] The Normal class's state** (`FUN_004cf6f0`): it too returns early on a zero colour alpha
  (594237, the BF1 twin of BF2's 723395). `TEST` = `0x50000` (594347), `0x5100b` when mode 1 (594350–351),
  `0x51001` when translucent (594358); translucency = *(decompiler detail omitted)* OR mode 2 OR *(decompiler detail omitted)*
  (594259) — the same three-term predicate as BF2, with the mode standing in for the bit; GS `ALPHA` `0x44`
  (594363) or `0x48` when `seg+0x30` (bit 0x80) is set (594364); AREF `| 0x600` (594425) = the same 0x60.
  **The translucent word's guard is looser than "translucent" alone** (594354–594358, sharpened after the
  verifier pass): `0x51001` is taken whenever *(decompiler detail omitted)* is set, translucent or not, and is NOT taken
  for a translucent segment when *(decompiler detail omitted)* is set (594356 jumps past the assignment, leaving
  `0x50000`). Neither bit is set on a soldier draw's flags, so the answer above is unaffected. **From the bytes:** `004cfb4c: lui r20,0x0005`, `004cfb68: ori r20,r20,0x100b`,
  `004cfb90/94: lui r20,0x0005 + ori r20,r20,0x1001`, and `004cfd3c`/`004cfd5c: addiu r4,r0,0x0600`.
- **[V][O] ABSENCE, searched:** in `FUN_004cf6f0` the palette arguments are referenced 2× each
  (594245–594246, 594498–594499) and reach only `FUN_0039bca0`. The word `0x5100b` occurs **exactly twice**
  in the whole BF1 corpus — 594351 (`FUN_004cf6f0`) and 594639 (`FUN_004d0108`, the ELF word at
  `0x0055e340` = the same Normal vtable + 0x30, which `FUN_004cf6f0` delegates to at 594248 when
  *(decompiler detail omitted)*, the sky flag) — and `0x5000b` occurs **zero** times. **Neither `0x5100b` nor
  `0x51001` occurs anywhere in the Specular class's draw `FUN_004b2310`**: that class never enables the
  alpha test and never takes the translucent word, which is why BF1's four `0x31` soldier segments are
  opaque whatever their sheet holds.

## 5. (c) The colour a soldier draw passes

In `FUN_00239bb0` (BF2's soldier draw) the colour buffer is built at lines 153660–153672 [V][O]:
*(decompiler detail omitted)* → *(decompiler detail omitted)* (the ALPHA byte), and
*(decompiler detail omitted)* → *(decompiler detail omitted)*
— **R, G and B are the same byte**, a brightness scalar (*(decompiler detail omitted)*, 153630–
153631). So the disc's soldier vertex colour is a grey level, **not** a team or damage tint [V][O]; nothing
else in that function writes those four bytes (the only other writes are the 16-byte vector copies at
153514/153567/153619, before the byte stores).

*(decompiler detail omitted)* is Ghidra's name for the float argument (`$f12`), which the thunk does not set — it comes from the
caller. Three call sites of that slot supply it, all with the literal **1.0f** = `0x3f800000` [V][O]:
- `FUN_005ace40` line 743999 — *(decompiler detail omitted)*, the **main draw after the LOD choice** (the loop above it picks *(decompiler detail omitted)* ∈ 0..3 by comparing a
  projected size against four thresholds);
- `FUN_005b0330` line 746652 — *(decompiler detail omitted)*;
- `FUN_005b0ab8` line 746873 — *(decompiler detail omitted)*.

The fade lives beside the second one: line 746659 calls the same slot under
*(decompiler detail omitted)* with *(decompiler detail omitted)* clamped to 1.0. **Note the decompiler
dropped the float argument in the printed call at 746659** (it shows *(decompiler detail omitted)*), the same omission class
`re-water.md` §3b found in the ocean loop; the fade value is the first argument there.

Consequences [D][O]: a soldier at normal range has colour alpha `0xff`, so *(decompiler detail omitted)* is false and his opaque
segments take `TEST 0x50000`; under the fade flag the colour alpha drops and **every** segment of him
becomes translucent (`ALPHA 0x44`, no Z write) — which is what the port's existing
`fs_mask_skinned_fade` twin approximates. BF1 tests the identical *(decompiler detail omitted)* term at 594259, so
the mechanism is the same; **BF1's soldier-side origin of that float is not traced** — see §7.

## 6. What the port should do (queue SKA's build half)

1. The skinned pipeline's alpha test should not exist. `SKINNED_MASK_CUTOFF` = 0.5 has no disc counterpart:
   the disc alpha-tests no soldier segment on either disc (§1), and the world's `MASK_CUTOFF` = 0.753 is
   for 0x02 segments, of which side soldiers have none.
2. A segment whose `MTRL` word carries 0x04 should BLEND — GS `ALPHA 0x44` with `As` = sheet alpha × colour
   alpha and **no depth write** (`TEST 0x51001`), drawn after the opaque segments. Today that is one
   segment per game; with P7's vehicles and the hero units it becomes 39 BF2 / 25 BF1 models (§1).
3. The verdict must be **per segment, not per texture**: all three `all_inf_wookie` segments name the same
   sheet and the disc draws two of them opaque and one blended. `bake::skin_glb` already writes one
   material per segment (`skin_glb.rs:159`), so `"alphaMode": "BLEND"` on the flagged one is the whole bake
   change, and `game/src/assets.rs:1023–1035` already maps a primitive's `alphaMode` to a renderer pass.
   **Corrected 2026-09-24 (SKA's build, lane 1) [V]:** true of BF2 only. Measured with `mungefs`'s
   `segdump` example over both `SIDE/ALL.LVL`s (`lead_tmp/skinned-alpha/probe/`): BF2's segments are
   `0x10001` / `0x10005` / `0x10089`, all three naming `all_inf_wookie`; BF1's are `0x1` naming
   `wookieface02` (the face, 227 vertices), `0x1` naming `wookie` (734) and `0x5` naming `wookie` (261). So
   in BF1 the fur shares its sheet with ONE opaque segment, not two. The conclusion stands in both games:
   an opaque and a blended segment name the same sheet, so the verdict has to be per segment.
4. `bake::alpha`'s force-opaque rule for unflagged side textures stays right (it is the disc's "the alpha is
   ignored" in the only place the port can express it), and so does keeping the wookie sheet's alpha.

**BUILT 2026-09-24 — queue SKA's build half, `ff71352`** (PLAN Log 2026-09-24): items 1–4 as written.
`bake::soldier::segment_blends` reads a skinned segment's `MTRL` word — 0x04 → `BLEND`, neither bit →
no `alphaMode` — and refuses the words the skinned path cannot draw (0x02, and 0x04 with 0x08 / 0x40 /
0x80), so a P7 vehicle or a hero unit that needs a skinned alpha test or another blend fails its bake
loudly instead of drawing wrong. The rebake moved exactly the two `all_inf_wookie.glb`; the goldens,
exactly the two `units/bf*_all_inf_wookie.png`.

## 7. Self-checks, the verifier, and what stays [H]

Self-checks run (`OPUS_LOOP_PROMPT.md` §5): **(1)** every cited line was re-read with its enclosing
`/* ==== addr FUN ==== */` header printed (`lead_tmp/re/side-alpha/fnof.sh`) — that is how §3's
posed-rigid/softskin correction to `re-material-flags.md` §1 was found. **(2)** Every absence claim is a
search with counts, listed in §3 and §4 (call-site counts, data-word scans of both ELF images, corpus-wide
constant counts). **(3)** The BF1 twin was opened function by function, including its own class label,
class vtable, instance ctor, `RedModel` vtable slot and four segment classes — BF1 does not mirror BF2
everywhere (a segment CLASS per flag bit dispatched virtually vs one draw that branches, `0x5100b` with
AFAIL FB_ONLY vs `0x5000b` with AFAIL KEEP, and a different posed-branch mask), though its model vtable
turns out to be structurally identical at the same slot +0xc — see §7's verifier paragraph, which corrected
this doc's first reading of that slot. The answer is the same in both games either way. **(4)** Every load-bearing constant was read out of the ELF: the two class-label
strings, five vtable words, the AREF byte, and the TEST/ALPHA immediates straight out of the instruction
stream in both games.

**Verifier: RUN, and it broke one claim.** One Opus agent was given the 25-claim brief
`lead_tmp/re/side-alpha/claims_for_verifier.md` (the citations alone, no reasoning, briefed to open every
cited line, name its enclosing function, and attack every absence claim with its own searches); its scratch
tooling — an independent R5900 disassembler, a `jal`/data-word/`lui`-pair scanner and an immediate census —
is in `lead_tmp/re/side-alpha/verify/`. **24 of 25 SUPPORTED; one NOT SUPPORTED, and it was right:**

- **BF1's model-draw slot was WRONG here** (claim B18): this doc first said vtable base `0x00558fe0`, slot
  **+0x14**. The agent found the base by its constructor stores instead of by eye — `&DAT_00558fe8` into
  object+0xc0 at `FUN_00496068:557008` and `FUN_004960e8:557038`, the only two address formations of it in
  the image — making the draw slot **+0xc**, identical to BF2's. Reproduced by the lead before acceptance
  (both lines read, `0x558fe8 + 0xc = 0x558ff4`) and §4 is corrected. The claim it damaged is the *rhetoric*
  that "BF1 does not mirror BF2's structure" — the vtables are in fact structurally identical; what really
  differs is the virtual dispatch of the segment draw and the posed-branch mask (below).
- **Every ABSENCE claim survived its attack, at the instruction level**, which is stronger than the textual
  counts this doc's §3/§4 rest on: 3 `jal 0x0058ed70` and **0** of the word anywhere in the 5,283,488-byte
  BF2 image (including the VU overlays and section headers) and 0 `lui`-pair formations; the palette's two
  stack slots (`340(sp)`/`344(sp)` BF2, `196(sp)`/`200(sp)` BF1) touched by **5** and **4** instructions in
  total, all prologue stores plus the one DMA call; `0x5000b` built in exactly 3 code sites in BF2 and
  **0** in BF1; `0x5100b` in exactly 2 BF1 sites, one per named function; and **0** formations of either
  word in BF1's Specular draw, which sets `0x50600` instead (573374/573377 — AREF pre-merged, ATE clear).
  It also chased the one `0x0005000b` data word in each image to its real owner: a Huffman
  `(code, nbits)` pair read by the bitstream writer, not a GS word.
- **Three line numbers in the brief were off and are corrected here:** the BF2 class-vtable store is
  169332 (not 169331), and BF1's instance allocation and ctor call are 123696/123697 (not 123690). All
  three re-read by the lead.
- **Four more dropped-argument sites**, the same decompiler failure mode `re-water.md` §3b hit: the
  `FUN_00239bb0` thunk prints the object in the *float* slot and drops `$f12` and `$a2` entirely (the
  prologue proves *(decompiler detail omitted)*, *(decompiler detail omitted)*); `FUN_004aa9f8`'s 9th argument is forwarded on the stack
  (`lw a0,176(sp)` / `sw a0,0(sp)`) but appears in neither signature; line 594248's zero-argument
  delegation is really a 7-argument forward; and `FUN_00589d18` takes an argument the decomp prints but the
  callee declares `void`.
- **Two structural differences it added** [V]: BF1 dispatches the per-segment draw *virtually* through the
  segment's own class (`*(seg+0x10) + 0xc`, §4) where BF2 calls `FUN_0058ed70` directly — the class
  hierarchy was collapsed between the games — and the posed branch masks `~0x01000000` in BF2 (712920)
  against `~0x00800000` in BF1 (557363).
- **One nuance on §5** [V]: *(decompiler detail omitted)* is re-scaled before the ×255 on three paths inside `FUN_00239bb0`
  (153353 *(decompiler detail omitted)*, 153383 *(decompiler detail omitted)*, 153583–589 *(decompiler detail omitted)* with a 0.06 floor),
  so the alpha byte is (possibly-scaled float) × 255 rather than the incoming float × 255. The 1.0f of the
  normal path is unaffected; a scaled path only makes a soldier MORE translucent, never opaque.
- **One caveat it recorded honestly**, and this doc adopts it: the data-word and `lui`-pair scans cannot
  exclude an address computed as `addiu rX, rY, delta` from an unrelated base, or a pointer written into
  `.bss` at runtime. No such pattern was found; the absence claims are exhaustive up to that limit.

The `[O]` tag still stands — one Opus pass plus one Opus verifier, no Fable audit — and SKA's build gates
(an exactly-2-file rebake diff, exactly two moved unit goldens) remain the independent check on the two
load-bearing census facts.

**What stays [H] / untraced:**
- **[H]** The soldier's draw colour alpha in **BF1**: the byte is tested at 594259 exactly as in BF2, but
  BF1's soldier→`RedModel::draw` chain was not walked to its caller, so the float's origin there is
  inferred from the BF2 twin, not read.
- **[H]** What `0x10000` (set on every BF2 word and no BF1 word) and `0x10` (`0x10010`, one BF2 soldier
  segment) do to a soldier in practice — `re-material-flags.md` §2 reads them as a no-cull VU flag and the
  dim-immune bit; neither changes the alpha verdict.
- **[H]** The per-object flags word the soldier draw ORs into *(decompiler detail omitted)* (*(decompiler detail omitted)* = obj+0x44 at the
  three call sites in §5) is not resolved statically. It cannot turn an alpha test ON — only `seg+0x18 & 2`
  does that, and no soldier segment has it — but a bit `0x800000` in it would force that object's segments
  translucent, so "opaque" above is the common case, not a proof for every object state.
- **[H]** Whether the disc's two OPAQUE wookie segments and its one BLENDED segment are drawn in that
  order, or sorted by the bucket the word selects (bucket 0xc for translucent, 723420). The port must
  choose an order; drawing opaque-then-blended is the conventional reading. **Ported 2026-09-24 (queue
  SKO, `662ba97`):** every opaque draw, world and skinned, goes before one blend pass that sorts the world's
  and the skinned BLEND draws far to near together (`render::frame::blend_order`); the order stays [H].
- Not asked and not answered: the `.dtl` detail sheets' alpha (a separate layer, `bake::detail`).

## 8. Reproduction

```
python3 engine/artifacts/texture_census/mtrl_census.py --out engine/artifacts/lead_tmp/re/side-alpha/side_rows.json \
    extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/*.LVL extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/*.LVL
python3 engine/artifacts/lead_tmp/re/side-alpha/census_a.py        # raw, per LVL set
python3 engine/artifacts/lead_tmp/re/side-alpha/census_b.py        # deduplicated, the §1 table
python3 engine/artifacts/lead_tmp/re/side-alpha/side_tex_alpha.py  # the shipped sheets' alpha
engine/artifacts/lead_tmp/re/side-alpha/fnof.sh analysis/SWBFRNT2.ELF-decomp.c 712870 712920 712952
python3 engine/artifacts/lead_tmp/re/side-alpha/elfrd.py extracted/bf2/SWBFRNT2.ELF 64febc 2
python3 engine/artifacts/lead_tmp/re/side-alpha/findword.py extracted/bf2/SWBFRNT2.ELF 0058ed70   # 0 hits
python3 engine/artifacts/lead_tmp/re/side-alpha/findstr.py extracted/bf2/SWBFRNT2.ELF soldier
```
