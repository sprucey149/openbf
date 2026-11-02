# LEAD VERIFICATION (Fable lead, 2026-09-03, session 12): re-read in both decomps before acceptance — BF2 `FUN_0058ed70` 723411–723496 (the translucent term, ALPHA 0x44/0x48/0x64, TEST 0x51001/0x5000b/0x50000, the 0x800800 gate), the MTRL→`seg+0x18` copy (724209–724232, 724334, 724811–724826, 724860), the 36 slot-+0xc callers (grep), the fade gate 712818–712828 / 712862–712870, the TEX0 words 717178–717188; BF1 class dispatch 568105–568160, mode bits 568925–568951, Normal 594259/594346–594373 (+0x600 AREF at 594425), Scroll 572568/572594–572602, Specular 573363–573374/573452–573454, TEX0 562030–562040, the Scroll/Normal vtables (0x559408 / 0x55e310, slot +0xc = 4b0c48 / 4cf6f0), BF1 sky variant 594636–594648 with AREF `DAT_00523024` = 0x40; ELF bytes 0x64feb0 (slot 3 = 0057cf60), 0x60c64c = 0x60. The one claim not re-derived by the lead is "RTYP dropped by `FUN_005901b0`" (the agent's Ghidra listing; `FUN_005901b0` is declared with argument 2 = RTYP at 724042 and the decomp never names argument 2 again — consistent). Lead's own measurement added: the baked textures' max alpha is 255 on 2,156 of 2,182 (one at exactly 128), so the decoder's full-range 0..255 reading of the pal_ bytes is the data's; the GS's 0x80-is-opaque blend arithmetic (Cv = ((Cs−Cd)·As >> 7) + Cd) is a hardware fact this doc's §2 relies on and is NOT yet reconciled with a full-range CLUT — one of the open items below. **RECONCILED 2026-09-09 (`analysis/re-hud-icons.md` citation 3, lead re-read B:571177–571201): the `pal_` loader scales every CLUT alpha (A·0x81)>>8, so disc 0xff = GS 0x80 = 1.0; the port's full-range alpha is the right blend factor, and the alpha-test cutoff in disc terms is ≈192/255 (queue item AT).** Tags and text below are the RE agent's.

# `segm MTRL` flag word → GS state: the general model draw path, both games (2026-09-03, Fable s12)

Tags: [V] read in the decomp / ELF bytes / disc bytes, [D] derived, [H] hypothesis. Decomp line numbers:
BF2 = `analysis/SWBFRNT2.ELF-decomp.c`, BF1 = `analysis/SWBFRONT.ELF-decomp.c`. Scratch (disassemblies,
pointer scans): `engine/artifacts/lead_tmp/s12/re/mtrl_scratch/`.

## §0 Headline

1. **Yes — `FUN_0058ed70` is THE per-segment draw for every `RedModel` in BF2** [V]: `RedModel` vtable slot +0xc
   (`0x64febc` = `0057cf60`, ELF bytes) is called from 36 sites — soldiers (0x80001), weapons, sky (0x20031),
   UI models (0x4000091), shadows — and `FUN_0057cf60` reaches `FUN_0058ed70` on all three of its branches
   (rigid 712870, skinned 712920, bone-attached 712952). There is no other material→GS rule in BF2.
   **Corrected 2026-09-23 (cleanse):** the labels are swapped — 712920 passes 0/0 for the palette (the
   POSED-RIGID branch) and 712952 is the palette (softskin) branch (`re-side-alpha.md` §2, verifier-checked).
2. **The word tested is the on-disc `MTRL` u32 verbatim** [V]: BF2 loader 724209 → `FUN_005916b0` 724860
   (`seg+0x18 = flags`); BF1 loader 568287 → `FUN_004abde0` 568927 (`seg+0x38 = flags`, plus 0x8000 if COL0).
3. **BF2 rule** [V 723411–723480]: translucent = (c.a≠0xff) | bit 0x04 | draw-flag 0x800000 → ALPHA 0x44
   (0x48 if 0x40, 0x64+FIX=a/2 if 0x08), TEST 0x51001 (Z test, NO Z write), bucket 0xc; else bit 0x02 →
   TEST 0x5000b (ATST GEQUAL, AREF 0x60 = 0.376, AFAIL KEEP); else 0x50000. 0x01 = lit, 0x80 = specular
   pass, 0x100 = env-map pass, 0x0f000000 = UV-animation type. **BF2 `RTYP` is parsed and dropped** [V disasm].
   **Corrected 2026-09-23 (cleanse):** the §0.6 erratum's "disc alpha ≥ 193" applies here too — with
   Av = 0x80 the AREF 0x60 test keeps disc alpha ≥ 193 (≈ 0.753); the port's world MASK has used exactly that
   since queue AT (`render/src/shader.wgsl` `MASK_CUTOFF`), and only skinned draws keep 0.5.
   **Corrected 2026-09-24 (cleanse, lane 1) — the line above is wrong twice:** the tested vertex alpha is
   Av = **0x7f** (the opaque vertex colour's `(0xff·0xff >> 8 + 1) >> 1`, as `MASK_CUTOFF`'s comment derives and
   `bake::detail::ALPHA_TEST`'s test pins for all 256 alphas) — Av = 0x80 would keep disc alpha ≥ **191**, not 193;
   and since queue SKA (`ff71352`, 2026-09-24) no skinned draw is alpha-tested at all (`re-side-alpha.md`).
4. **BF1 has FOUR segment classes chosen by flag bits, not by RTYP** [V 568105–568160]: 0x20 Specular, 0x400
   Scroll, 0x800|0x4000 Blur, else Normal; the RTYP string is hashed and never forwarded. Normal: 0x02 →
   TEST 0x5100b (AFAIL **FB_ONLY**, no blend), 0x04 (w/o 0x02) → blend 0x44/0x48 + 0x51001; Scroll: 0x02
   ignored, 0x04 → blend; Specular: 0x02 and 0x04 both ignored (opaque + additive spec pass by Ad).
5. **Texture alpha ALWAYS reaches the blend/test**: TEX0 TCC=1 (RGBA) and TFX=0 (MODULATE) for every texture
   on both games [V BF2 717180–717187/717400–717470, BF1 562033–562038]; vertex alpha = 0x80·c.a/0xff. So a
   0x04 segment blends with As = texture alpha × colour alpha, never colour alpha alone.
6. **Contradictions with ALPHA RULE v2**: (a) BF1 `kam_crate` body 0x05 → ELF BLENDS it by its grime alpha
   (port OPAQUE); (b) class D (banner/rope/deck edge/grate, words 0x1/0x10001) → ELF OPAQUE, no test (port
   MASK); (c) `myg1_mountains` 0x03 → ELF alpha-TEST at 0.376 (port BLEND, rule 1); (d) MASK cutoff 0.376 not
   0.5.
   **ERRATUM (2026-09-17, the AT landing): (c) and (d) state AREF 0x60 in DISC alpha terms and that conversion is
   wrong — it predates the CLUT scale.** The `pal_` loader uploads every CLUT alpha as `(A·0x81)>>8`, so AREF 0x60
   is disc alpha **≥ 193**, not ≥ 96/255 = 0.376 (`RE_LOG.md` "Alpha-test cutoff in disc terms", ported as
   `MASK_CUTOFF = 0.753` in `a55688f`). Consequences for the rows below: `myg1_mountains` amax 115 reaches the GS
   as `(115·129)>>8 = 57`, so it would pass NOWHERE under the test, not "where α ≥ 96" — the port routes it BLEND
   by rule 1, so no port behaviour turns on it, but the table's reading of that row is void; and `mus1_sky_volcano`
   "MASK@0.376" is the same arithmetic. The GS-side readings (TEST word, AREF, TFX, TCC) are unaffected: only the
   disc-alpha translation was wrong. Agreements: leaves/bamboo 0x07 MASK, fountains/mist/waterfalls/shield 0x414/0x…05 BLEND, Scroll 0x410 and
   Specmap 0x31 strips OPAQUE, `kas2_prop_light` 0x10010 OPAQUE, BF2 crate 0x10089 OPAQUE.
7. **Recommendation: replace the census heuristic with the ELF rule (per game, per class) behind a bake flag,
   and settle (a)/(b) with a PCSX2 capture** — §6 gives the exact GS state per named check; the census's
   thresholds encode the eye, the ELF encodes the disc, and they disagree on precisely the cases the lead flagged.

**Applied to BF2 2026-11-01 (`bake::alpha::bf2_elf_verdict`), point 6(b):** a BF2 word with neither 0x02 nor 0x04 draws
OPAQUE, no test. The census had cut those draws out on 15 BF2 maps (153 baked files change; collision and manifests do
not) and in BF2's common bake (`hud_lightsaber`, `hud_force_saberthrow` and both holocron models). The HUD composites
`hud_*` art by sheet alpha whatever the word, so those sheets keep it (`bake::map::content`'s `hud_named`). Checked against existing PCSX2 walkabout frames, not a new capture: `bf2_yav1/walk/pairs/cp4_bazaar` (the obelisk
is plain stone, F47), `bf2_dea1/walk/pairs/cp2_cp5` b090 (the red console is whole). No PS2 frame exists inside DAG1's
cave, whose wall (`dag1_prop_cave_C`, 0x10201) was the report; its result is the ELF rule plus port renders. BF2 words
with 0x04 stay the census's (card ELFR2, unchanged).

## §1 Q1 — is `FUN_0058ed70` reached for world models? [V]

- `RedModel` vtable `DAT_0064feb0`: slot 3 (+0xc) = `FUN_0057cf60` (`elfrd.py bf2 64feb0 6` → `0064febc: 0057cf60`).
  Ctor sites 712517/712549 install it; RTTI "8RedModel" at 0x64fec8.
- `FUN_0057cf60(model, M, bones, colour, flags, attach)` 712624: per-segment sphere cull (712650–712720, far =
  `FUN_00565140` when flags&0x20000 else −1); `flags |= 0x10` when `DAT_0060c2b5`==0 (712758–712760); lighting
  block skipped when `(flags & 0x80000010)` (712762); colour copied to a local (712797–712800); **distance fade
  on the local alpha unless flags&0x4020000** (712818–712828); then the segment loop (712840–712956):
  - rigid: `FUN_0058ed70(seg, M, 0, 0, flags, colour*, attach)` 712870;
  - skinned (`bones != 0`, per-bone matrix): 712920 with `flags & ~0x1000000`;
  - segments with `seg+0x84 != 0` (bone-attached/cloth): `FUN_0057f158`/`FUN_0057e780`/`FUN_0057e8e8` then
    `FUN_0058ed70(seg, M, palette, n, flags, colour*, attach)` 712952.
  - `colour*` = the FADED local only if `(seg+0x18 & 4)` or the caller's alpha ≠ 0xff, else the caller's colour
    (712862–712867, 712915–712921, 712948–712952). **The distance fade applies only to 0x04 segments.**
  - `(flags & 0x230000) == 0x10000` → `FUN_00592c38(model+0xb8, …)` 712966 (a sub-model list; not a material path).
- 36 callers of slot +0xc (scratch `findptr.py`/grep `0xc0) + 0xc))`), with the flags word they pass:
  | site | fn | flags | colour |
  |---|---|---|---|
  | 68392/68715/68763 | `FUN_001c0688/0d38/0dd8` | `p|0x80001` (bucket 0xe) | white `0x76c200` |
  | 72102 | `FUN_001c5610` | pass-through | caller's, alpha×fade (72095) |
  | 85366 | `FUN_001d7cb0` | `p|8` | local |
  | 130065 | `FUN_002186c8` | `p|0x10` (unlit) | alpha×(obj+0xd0)×(cls+0x80)×dist (130059) |
  | 155279 | `FUN_00239bb0` ("player_shadow" refs) | *(decompiler detail omitted)* | local |
  | 197829 | `FUN_00278088` | *(decompiler detail omitted)* | *(decompiler detail omitted)* |
  | 305120/308861/313830 | `FUN_00314768/319c98/320b70` | *(decompiler detail omitted)* | obj+0x44 / white / white |
  | 319189/321318/325843 | `FUN_003278a8` (Weapon vtables 0x627e3c…) | *(decompiler detail omitted)* | *(decompiler detail omitted)* |
  | 321222 | `FUN_0032b0e0` | 0 | local |
  | 333912–334037 / 337515–337642 | `FUN_0033e018` (vt 0x6298cc, WeaponMelee region) / `FUN_00343d78` | `p|1` / *(decompiler detail omitted)* | *(decompiler detail omitted)* / *(decompiler detail omitted)* |
  | 374234 | `FUN_00377cc0` | 0x41… | float colour |
  | 378477/379314 | `FUN_0037f110/0037fd30` (vt 0x62d9b8/0x62db10) | 0x4000000 | *(decompiler detail omitted)* |
  | 417535 | `FUN_003bf060` (SkyObject) | `p|0x20001` | alpha=|f|·255 (417530) |
  | 425851–426008 | `FUN_003cb918` (sky dome) | 0x20031 / 0x2000031 | sky colour tables |
  | 487147 | `FUN_0042c788` | `p|0x1000000` (0x80 toggled 487139) | obj+0x60 |
  | 705565 | `FUN_0056fae0` | 0 | white |
  | 734792/735190 | `FUN_0059f6c8/0059fe60` (Red3DModelElement vt 0x650950/0x6509b0) | 0x4000091/0x4000081 (735097) | *(decompiler detail omitted)* |
  No caller passes a different material rule: the segment-level rule is decided inside `FUN_0058ed70` from
  `seg+0x18` and the flags/colour it receives. Terrain, water and particles are not `RedModel`s [D: none of the
  36 sites is the terrain renderer; terrain has no `segm`].
- Draw-flag (argument 5) bits read by the pair [V]: 0x1 (FUN_00497570 mask 0x100, 724027–724030), 0x10 unlit,
  0x20 (sky Reflect: cutouts become blended 723468–723470, TEST 0x51001), 0x80 (lighting arg 712775), 0x20000
  sky (bucket 8/9, far cull, no fade), 0x80000 bucket 0xe, 0x800000 force translucent+additive, 0x1000000
  cleared on skinned, 0x2000000 bucket 3, 0x4000000 bucket 0x12 + no cull/fade/dim (UI), 0x80000000 no lighting.

## §2 Q2 — the segment flag word, bit by bit

### BF2 loader → `seg+0x18` [V]
`FUN_00590328` 724209–724230: `MTRL` = u32 flags (*(decompiler detail omitted)*), u32 diffuse RGBA, u32 specular RGBA, u32
specular power, two u8 (*(decompiler detail omitted)*), a string. `RTYP` u32 → *(decompiler detail omitted)* (724284). Call 724334:
`FUN_005901b0(power, RTYP, &INFO, bone, MNAM, TNAM[4], flags, &diffuse, &spec)`. Disassembly
(`mtrl_scratch/bf2_590328_tail.txt`, `bf2_5901b0.txt`): RTYP goes in `a0` (`lw a0,0x300(sp)` = *(decompiler detail omitted)*) and
`FUN_005901b0` clobbers `a0` at its second instruction (`lbu a0,0x110(sp)`) — **RTYP is never stored**.
`FUN_005916b0` 724796: `seg+0x04` prim word = 0x4000 if flags&1 (724813), |0x100000 if COL0 (724819),
|0x2000 if flags&0x200&&1 (724822–724826); `seg+0x18 = flags` verbatim (724860); diffuse floats +0x1c..+0x28,
spec +0x2c..+0x38, power +0x3c; TNAM slots → +0x08 (0), +0x10 (1), +0x0c (2, bump), +0x14 (3, env).
`FUN_0058e730` 723062 (the two u8): `flags & 0x0f000000` selects the UV-animation type — 0x1000000 → rates
+0x18/+0x19, 0x2000000 → +0x1a/+0x1b, 0x4000000 → angle (+0x1c/+0x1d, +0x1e), 0x8000000 → +0x10; none → a
static UV scale (723073–723095). [D] BF2's "RTYP 3" is redundant with 0x1000000 (census: every rtyp-3 word carries it).

### BF2 draw `FUN_0058ed70` — segment word `f`, colour `c`, draw flags `F` [V]
| bit | effect | lines |
|---|---|---|
| 0x01 | LIT: prim 0x4000 set at load; cleared at draw if bit clear, or F&0x10, or `DAT_007c5768`==0 (723489–723491); gates the light/env second pass (723800) | |
| 0x02 | ALPHA-TEST: TEST 0x5000b = ATE, ATST GEQUAL, AFAIL KEEP, ZTE, ZTST GEQUAL (723478–723479); AREF = `DAT_0060c64c`<<4 = **0x60** (723553/723598; ELF bytes `0060c64c: 00000060`); with F&0x20 treated as translucent (723468–723470) | |
| 0x04 | TRANSLUCENT: bucket 0xc (723420); ALPHA 0x44 = Cs·As+Cd·(1−As) (723453); TEST 0x51001 = ATE, ATST NEVER, AFAIL FB_ONLY, ZTE GEQUAL → **Z-tested, no Z write** (723466/723475); vertex alpha = c.a·0xff>>8 (723449); prim 0x800800 dropped when faded by colour (723484–723486); selects the distance-faded colour in `FUN_0057cf60` (712867); spec pass uses As instead of Ad (723895–723898); with 0x10 skips the global dim (723500) | |
| 0x08 | if translucent: ALPHA 0x64 = (Cs−Cd)·FIX+Cd, FIX = a/2 (723455–723458) | |
| 0x10 | dim-immune: with 0x04 skips `DAT_007c5754` scaling (723500–723504); if dim<1 and !0x04 → ALPHA 8, colour 0xff404040 (723544–723549). `DAT_007c5754` is 1.0/0.5 from `FUN_00456098` (516755–516765) [H: pause/menu dim] | |
| 0x40 | if translucent: ALPHA 0x48 = Cs·As+Cd additive (723450–723452, 723460) | |
| 0x80 | SPECULAR second pass: record 0x7c57b0 (ALPHA 0x58 = Cs·**Ad**+Cd, FIX 0xff, TEST 0x5001b) or 0x7c57c0 with 0x04 (ALPHA 0x48 = Cs·As+Cd) (723880, 723895–723910; table init 724935–724945); highlight texture from power `seg+0x3c` via `FUN_00589ac0` | |
| 0x100 | ENV-MAP second pass with TNAM slot 3 (`seg+0x14`) or the global env (723879, 723912–723945): record 0x7c57d0 = ALPHA 0x68 = Cs·FIX+Cd, FIX 0x80 (additive) | |
| 0x200 | with 0x01 and COL0: prim |= 0x2000 (724822–724826) [H vertex-colour lighting] | |
| 0x1000 | bucket 0xb (723418–723423) | |
| 0x10000 | suppresses prim 0x800800 for 0x02/0x04 segments (723484) [H two-sided/no-cull VU flag; BF2 words all carry 0x10000, BF1 words none] | |
| 0x0f000000 | UV-animation type (above); at draw `seg+0x40`≠0 → UV matrix program 0x7c58f0, prim |= 0x100 (723541–723600) | |
| 0x20, 0x400, 0x800, 0x4000, 0x8000 | **unread** by `FUN_005916b0`/`FUN_0058e730`/`FUN_0058ed70` (grep over 712000–726500: no `& 0x400`) [V negative] | |
Also: `seg+0x0c` (TNAM 2) with `DAT_0060c650`=1 → bump-map pass (723695–723790); prim 0x8/0x18 = IIP/IIP|TME,
|0x40 (ABE) iff an ALPHA value is set (723492–723496, 723561–723563); TEST gets |0x30000 when no ZTST (723556).
**Corrected 2026-09-24 (queue SPEC's RE, lane 1, `[O]`):** the 0x80 row's "highlight texture from power" is
wrong. `FUN_00589ac0` builds a texture-coordinate MATRIX, and the pass samples the disc's one `specular` table
(CORE.LVL) at s = power/255, t = 0.5 + 0.5·(L̂·R), with R the VU1's view-space reflection vector. The pass colour
is flat. Its alpha byte is 1, so the 0x04 record (Cs·As + Cd) is invisible, and far-scene draws skip the pass in a
mission. See `analysis/re-specular-pass.md`.

### BF1 loader → `seg+0x38` and the class [V]
`FUN_004aacd8` 568165: `MTRL` u32 flags 568288; bit 0x20 → two u32→float, else bit 0x10 → two floats
(568289–568305); `RTYP` string hashed (568405–568408) → arg 1 of `FUN_004aa9f8` (568455) which never
forwards it (568158: `FUN_004abde0(obj, p2..p8)`). Class by flag bits 568105–568160: **0x20 →
ps2SpecularSegment** (vt 0x5594a0, draw `FUN_004b2310`), **0x400 → ps2ScrollSegment** (vt 0x559408, draw
`FUN_004b0c48`), **0x800|0x4000 → ps2BlurSegment** (vt 0x55e278, draw `FUN_004cf0a8`), **else (incl. 0x200) →
ps2NormalSegment** (vt 0x55e310, draw `FUN_004cf6f0`) — vtables read with `elfrd.py`, RTTI strings follow each.
The two floats go to slot +0x1c (568457–568459): scroll rates (`+0x3c/+0x40`, used 572620–572650).
`FUN_004abde0` 568903: `+0x38 = flags` (568927; loader adds 0x8000 when COL0, 568446); prim `+0x28` = 0x4000 if
0x01, |0x2000 if 0x40, |0x100000 if 0x8000 (568929–568935); `+0x30` = bit 0x80 (568938); **`+0x2c` mode = 1 if
0x02, else 2 if 0x04, else 3 if 0x08, else 0** (568939–568951).

### BF1 draw — per class [V]
- `RedModel::Draw` = `FUN_00496248` 557105: fade skipped when F&0x2020000 (557240–557252); `F|=4` when near
  (detail-texture pass) 557228–557237; the faded colour is used only when slot +0x2c (`FUN_004ac790` = `mode==2`,
  disasm `bf1_4ac780.txt`) or c.a≠0xff (557304–557311). Same shape as BF2.
- **Normal `FUN_004cf6f0`** 594179: F&0x20000 → sky variant slot +0x30 (`FUN_004d0108`, 594248); translucent =
  c.a≠0xff | mode==2 | F&0x400000 (594259); TEST: mode 1 → **0x5100b** (594351: ATE, GEQUAL, AREF 0x60 via
  `|0x600` 594425, **AFAIL FB_ONLY**), translucent or F&0x20 → 0x51001 (594353–594360), else 0x50000; ALPHA only
  when translucent: 0x44, or 0x48 if `+0x30` (0x80) or F&0x400000 (594362–594365); prim |= 0x800800 if translucent
  && mode 0 (594368–594370); F&0x10 clears 0x4000 (594371–594373); PRIM 0x18/0x58 (594429–594432). Mode 3 (0x08)
  has no effect beyond "not mode 0". Buckets: `FUN_0049e260` opaque, `FUN_0049e300` translucent, `FUN_0049e2c0`
  translucent+tested (594262–594330).
  **Corrected 2026-09-23 (cleanse):** add the `& 0x20000000` exception
  (`re-side-alpha.md` §7; BF1 decomp ~594356).
- **Scroll `FUN_004b0c48`** 572482: translucent 572568 (same term); TEST 0x51001 if translucent or F&0x20 else
  0x50000 (572594–572597) — **0x02 is never tested**; ALPHA 0x44/0x48 (572598–572602); UV offset = rates × time.
- **Specular `FUN_004b2310`** 573294: ALPHA 0x44 only if c.a≠0xff (573363–573366); TEST always 0x50600 (573374:
  ATE off) — **0x02 and 0x04 ignored**; second pass record `DAT_006f4540` = ALPHA 0x58 (Cs·**Ad**+Cd), FIX 0xff,
  TEST 0x50000 (573452–573454) with the highlight texture from the spec power (`FUN_004a82f8(+0x3c)`).
  **Corrected 2026-09-24 (queue SPEC, `[O]`):** `FUN_004a82f8` builds a tex-gen matrix into the shared
  `specular` table, as BF2 does. The pass colour is B (MTRL bytes 8..11) raw, or the SunInfo colour raw when
  B is 0 or white — `analysis/re-specular-pass.md` §2–§3.
- Sky variant `FUN_004d0108` 594508: mode 1 → ALPHA 0x44 **and** 0x5100b (594639–594647), AREF `DAT_00523024`=0x40.
- BF1 bits summary: 0x01 lit; 0x02 test (Normal only); 0x04 blend (Normal, Scroll); 0x08 mode 3 (no GS effect);
  0x10/0x20 "MTRL has two floats" (0x20 also = Specular class); 0x40 prim 0x2000 + light-set select
  (569068–569072, 594275–594278); 0x80 additive (0x48); 0x200 no effect (Normal); 0x400 Scroll class;
  0x800/0x4000 Blur class (no shipped word uses them); 0x8000 COL0 (loader). 0x100/0x1000/0x10000: unread.

### Texture function — where the texture alpha goes [V]
TEX0 is built once per texture at upload: BF2 `FUN_00582c90` 716966 — paletted: `| 0x400000000` (TCC=1 RGBA),
CLD via 0x2000000000000000 (717180–717187); 16/32-bit: every TH case ORs 0x400000000 (717400–717470). No 0x800000000/
0x1000000000 → **TFX = 0 MODULATE**. BF1 `FUN_0049c5c8` 561661, identical constants (562033–562038). Hence
fragment alpha = At × Av >> 7 with Av = (c.a·0xff>>8 +1)>>1 ≈ 0x80 for an opaque colour → **As = texture alpha**.
The only case where the texture alpha does NOT decide is a segment that is neither blended nor tested (TEST 0x50000,
no ABE): then it is merely written to the frame buffer's alpha — which the 0x80 specular pass reads back as Ad.

## §3 Q3 — the colour argument

BF2: the callers in §1 pass white (`0x76c200` = ff,ff,ff,ff, table init 712404–712440) or the instance's own
colour; alpha≠0xff arises only at named fade sites: effects `FUN_002186c8` 130059 (alpha × class × distance),
the fade wrapper `FUN_001c5610` 72095, SkyObjects 417530, the dome lerp (`sky+0x2855`), and the **per-segment
distance fade** 712818–712828 (`alpha × (1 − (d − (far − 0.75r)) / (fade + 0.75r))`) — which only 0x04 segments
receive (712867). [D] Buildings/props are drawn with alpha 0xff except at the far fade, so `c.a≠0xff` is the
exception; the material bit is what puts a world segment on the blend path. BF1 mirrors it (557248, 557304–557311).

## §4 Q4 — render type

- BF2: `RTYP` u32 parsed (724284) and discarded (§2). The dispatch that exists is on FLAG bits: 0x80 specular
  pass, 0x100 env pass, 0x0f000000 UV animation (`FUN_0058e730`), TNAM-2 bump (`DAT_0060c650`). None of them
  changes the ALPHA/TEST selection except 0x04 flipping the specular pass from Ad to As (723895–723898). The
  sky's reflect pass (F&0x20020==0x20000, 723621–723680) picks 0x7c5810/0x7c5820 by bit 0x02.
- BF1: the class IS the render type, chosen by flags 0x20/0x400/0x800|0x4000 (568105–568160); the `RTYP` string
  is dropped. Specular forces nothing additive on the base pass; its second pass is additive by Ad (573452).
  Scroll animates UVs (572620–572650) and drops the 0x02 test. [D] The munger wrote `RTYP` and the flag bits
  consistently ("Scroll" ⇔ 0x400, "Specmap" ⇔ 0x20, both with 0x10 = floats present), which is why the port's
  string reading has been a usable proxy.

## §5 Q5 — BF1 confirmed/refuted

Confirmed with differences: the BF1 sky tick (280279) reaches `FUN_00496248` → per-class draw. Same skeleton
(translucent = c.a≠0xff | 0x04-mode | force flag; blend 0x44/0x48; no-Z-write 0x51001; fade only on 0x04), but:
(i) the additive bit is **0x80 in BF1, 0x40 in BF2**; (ii) 0x08 halves alpha in BF2, is inert in BF1; (iii) BF1's
alpha test is **0x5100b (AFAIL FB_ONLY)** where BF2 uses 0x5000b (KEEP) — see §8; (iv) 0x02 is honoured only by
the Normal class; (v) BF1 0x10 = "two floats follow", BF2 0x10 = dim-immune; (vi) the BF1 force-translucent draw
flag is 0x400000 (BF2 0x800000); (vii) BF1 0x10000 unread (BF2: 0x800800 gate).

### §5.1 BFC scope reconciliation (2026-10-25; research incomplete)

The BFC poses report a dark surface where the reference sees a room, floor or
fog. The cause remains **[H]**. This section records current port source and
the limits of the earlier notes; it supplies no new verified PS2 cull rule.

- **Current port source:** `render/src/pipelines.rs:672–677` constructs a
  triangle-list pipeline with `cull_mode: None`. Its mixed-winding comment is
  not a measured count of flipped submeshes.
- **Current conversion path:** `mungefs/src/model.rs:403–443` finds strip
  boundaries from flagged pairs, masks the high index bit, drops repeated-index
  triangles and alternates winding by the strip triangle offset. The GLB writer
  flattens those triangles (`bake/src/glb.rs:114–115`) and writes the alpha
  record's flags (`:125`; `bake/src/alpha.rs:482–488,519–528` copies the raw
  segment flags); the runtime keeps flags per primitive
  (`game/src/assets/mesh.rs:316–323`). This does not establish authored facing,
  the PS2's strip seed/ADC behavior, or a shipped winding census.
- **Historical evidence, not re-derived here:** [re-vu1-cull.md](re-vu1-cull.md)'s
  lead header explicitly excludes its VU1 reading from verification. Its body
  describes a conditional cull rule, calls plain opaque segments two-sided and
  leaves the facing convention open. This document's BF1 Normal and BF2 draw
  sections also describe different gates. BFC's blanket BF1 culling premise
  cannot be promoted from those summaries. Re-read both games' callers, class
  dispatch, defaults/writers and VU1 output before specifying a per-draw rule.
- **Unresolved competing cause:** `render/src/camera.rs:39` defaults near to
  0.1 m; `game/examples/pose_render.rs:226–235` inherits it in the ordinary pose
  path (the zoom path can supply another camera). The RULES note from AUDIT
  VTURZ requires distinguishing a near-band surface from a culling difference.
  At each BFC pose, identify the occluding draw and its eye-space depth, then
  measure the reference camera/near plane and test the causes separately.

The original BFC pixel/parity bars remain unmeasured. A build specification and
golden move list require the affected-draw and winding census first.


## §6 Q6 — the named checks under the ELF rule

| check | word / class | ELF GS state | port v2 | agree? |
|---|---|---|---|---|
| `kam_crate` body (BF1) | 0x05 Normal, mode 2 | **BLEND** 0x44, As = grime alpha (TCC RGBA), TEST 0x51001 (no Z write), translucent bucket, distance-faded | OPAQUE | **no** |
| `kam_crate` Scroll strip | 0x410 Scroll, mode 0 | OPAQUE, UV scroll; alpha only written to FB | OPAQUE | yes |
| `kam_crate` Specmap strip | 0x31 Specular | OPAQUE + additive spec pass Cs·Ad+Cd (alpha = gloss mask) | OPAQUE | yes |
| BF2 crate | 0x10089 | OPAQUE (0x08 inert w/o 0x04) + spec pass by Ad | OPAQUE | yes |
| Endor leaves BF1 | 0x06/0x07 Normal, mode 1 | TEST 0x5100b AREF 0x60 (=0.376), no ABE, FB_ONLY (§8) | MASK 0.5 | yes (cutoff differs) |
| Endor leaves BF2 | 0x10002/0x10003 | TEST 0x5000b AREF 0x60 KEEP | MASK 0.5 | yes (cutoff) |
| bamboo 0x1 / 0x7 | Normal | 0x1 OPAQUE; 0x7 MASK@0.376 | OPAQUE / MASK | yes |
| banner, rope, deck edge, grate (class D) | 0x1, 0x10001, 0x10201 | **OPAQUE, alpha ignored** | MASK | **no** |
| `tat2_bldg_jawa_junky` 0x1, `cor1_bldg_warroom05` | | OPAQUE | OPAQUE | yes |
| `kas2_prop_light` 0x10010 | | OPAQUE (0x10 = dim-immune) | OPAQUE | yes |
| fountains 0x414, waterfall 0x414/0x415, mist 0x415, northern lights 0x414 | Scroll, mode 2 | BLEND 0x44, scrolled, no Z write | BLEND | yes |
| BF2 fountain2 0x1000005, waterfall 0x1010005, shield 0x1010004/5/14, screens 0x1000005/0x1010205 | | BLEND 0x44 (+UV anim), no Z write | BLEND | yes |
| console fx 0x410 / 0x414; BF2 0x1010000 / 0x1000004 | | OPAQUE-scroll / BLEND | OPAQUE / BLEND | yes |
| `bes2_prop_lamp_skirt` 0x1 / 0x414 | | OPAQUE / BLEND | OPAQUE / BLEND | yes |
| `myg1_mountains` 0x03 | mode 1 | **MASK** at 0x60: amax 115 ≥ 96 passes, so the mountains draw where α≥96 | BLEND (rule 1) | **no** (both visible) |
| `myg1_sky_mist` 0x1000005 / 0x5 | | BLEND / BLEND | BLEND / OPAQUE | half |
| `sky_naboo2` world (`nab2_bldg_rot_floor` 0x1 / 0x10001) and shell (0x20001 / 0x12001) | | OPAQUE both; shell also gets F=0x20031 (unlit, bucket 8, no fade) | OPAQUE / sky composite (D7) | shell differs |
| `mus1_sky_dome` 0x10001/0x10010, `mus1_sky_volcano` 0x10003 | | OPAQUE / OPAQUE / MASK@0.376 | D7 keeps alpha | as sky report |
| terrain sheet carrying 0x04 | some segm names it | the bit belongs to that segm; terrain has no `segm`, never consults MTRL [D] | per-draw already | n/a |

**The crux (crate):** the ELF gives the BF1 crate body ALPHA 0x44, PRIM ABE, TEX0 TCC=RGBA/TFX=MODULATE, vertex
alpha 0x80 → As = the texture alpha texel by texel; TEST 0x51001 (no Z write) in the sorted translucent bucket.
With a0 = 34 % of texels ≤ 31, a third of the crate body is ≥ 88 % see-through on the PS2 by this reading — a
"ghosted" crate, not a lattice (a lattice is the MASK reading). If the lead's PS2 reference shows a solid crate,
the disc texture the PS2 samples must differ from the census's decode (palette alpha / CLUT decode) — that is
the one thing this report cannot check without a capture. Nothing in the draw path makes a 0x04 segment opaque.

**Implication for the port** — where v2 and the ELF differ: (1) 0x04-alone segments (BF1 Normal/Scroll, BF2
all): ELF = BLEND regardless of histogram (v2 rule 3 sends them to MASK/OPAQUE by `mid`); (2) 0x02: ELF = MASK
at 0.376 in BF2 and in BF1's Normal class only (v2: any class, 0.5); (3) unflagged words: ELF = OPAQUE (v2 class
D → MASK); (4) `amax ≤ 128`: no rule in the ELF; (5) Specular class (0x20/0x31/…): ELF ignores 0x02/0x04 (v2
rule 2 can BLEND a 0x24-style Specmap with `mid ≥ 0.3`); (6) 0x10 alone: agree (opaque); (7) BF2 0x08 + 0x04 →
half-alpha blend (v2 has no such mode).
**COUNTED 2026-09-24 (queue NT, lane 1 — a port census, not new RE; report
`engine/artifacts/loop/reports/20260924_0345_build_land_NT.md`):** over the shipped bake's 6,155 rule-decided
materials, 602 differ from this section's reading of their word (`lead_tmp/never-tests/v2_vs_disc_all.txt`).
Point (4) is the cleanest: v2's rule 1 BLENDs 55 materials, and 37 carry a word with neither 0x02 nor 0x04,
which §2 draws OPAQUE — COR1's `Cor1_Bldg_security_roomDoor` ×24, DEA1's `deal1_bldg_reactor_room02` ×7, BF1
BES1's `kam_bldg_antenna` ×4, GEO1's `geo_sky` ×2 (a sky shell). Their alpha is a glow mask, not a cutout, so
the port draws those doors and walls nearly invisible; queued as R1U. Rule 1's threshold stays 128 rather than
following the world's test (disc alpha 193): at 193 it would flip 23 OPAQUE draws to BLEND, 15 of them words
§2 draws opaque (NT's decision).
**PORTED 2026-09-24 (queue R1U, lane 1 — `0c3b5c3`; report
`engine/artifacts/loop/reports/20260924_0430_build_land_R1U.md`):** point (4) for words with neither bit, and
point (3) where the texture has no texel past the world's test. Rule 1 now fires only on a word with 0x02 or 0x04,
and rule 3's class-D MASK needs alpha max ≥ 193 (below it that MASK drew nothing); the 37 words above are OPAQUE,
and so is `mus1_sky_dome`'s 0x10001 (class D, max 173 — the sky path never reads it). Census after the rebake: 602
→ 564 disagreements, ('1','BLEND','OPAQUE') 0. Point (3) stands for class-D sheets that pass the test (293 MASK
draws of words §2 draws opaque). With point (4)'s unflagged half gone, NT's reason no longer holds: on top of R1U
the raise to 193 turns exactly the 8 `Kam_bldg_window_01` 0x04 draws to BLEND, which is §2's reading, and moves
no other verdict — queued as KW.
**PORTED 2026-09-24 (queue KW, lane 1 — `bc069b9`; report
`engine/artifacts/loop/reports/20260924_0500_build_land_KW.md`):** point (1) for a 0x04 word whose sheet has no
texel past the world's test — rule 3's 0x04 clause returns BLEND below alpha 193 and keeps its histogram above it.
Exactly the 8 `Kam_bldg_window_01` draws (0x10005 ×1, 0x205 ×3, 0x10205 ×4; the sheet is glass, alpha 123..183,
mid 1.0) moved OPAQUE → BLEND; census after the rebake ('3','OPAQUE','BLEND') 68 → 60, disagreements 564 → 556.
Point (1) stands for 0x04 sheets that pass the test — the crate question above (60 OPAQUE and 103 MASK draws of
rule-3 words §2 blends). `NEVER_TESTS` stays 128: the raise would have moved the same 8 through rule 1, and would
also composite a 0x02 word in 129..192, which BF2's disc draws nothing of.
Recommendation: implement `verdict_elf(game, flags)` = the §2/§5 tables (BF1: class → mode; BF2: bits) with
AREF 96/255, no histogram; run the bake with it behind a flag and A/B the KAM1 crate, the Theed banner and
MYG1's mountains against PCSX2 frames. If the PS2 crate is solid, the discrepancy is in the texture decode, not
the rule, and the census's `mid` heuristic is patching the wrong layer. If the PS2 crate is ghosted, adopt the ELF
rule outright and drop rules 1/3's histogram terms (class D included). Either way the sky pipeline (D7) is
consistent with the ELF: shells with 0x10001 draw opaque, 0x10003 tested, and the far bucket never writes Z.

## §7 Reproduction
```
cd <repo>
sed -n 723411,723496p analysis/SWBFRNT2.ELF-decomp.c        # BF2 material→GS selection
sed -n 712818,712828p analysis/SWBFRNT2.ELF-decomp.c; sed -n 712862,712870p analysis/SWBFRNT2.ELF-decomp.c
sed -n 724209,724232p analysis/SWBFRNT2.ELF-decomp.c; sed -n 724334,724336p analysis/SWBFRNT2.ELF-decomp.c
sed -n 724811,724826p analysis/SWBFRNT2.ELF-decomp.c; sed -n 724860p analysis/SWBFRNT2.ELF-decomp.c
sed -n 717178,717188p analysis/SWBFRNT2.ELF-decomp.c        # TEX0 TCC/TFX (BF2)
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 64feb0 6   # slot 3 = 0057cf60
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 60c64c 1   # AREF 0x60
grep -n "0xc0) + 0xc))" analysis/SWBFRNT2.ELF-decomp.c | wc -l                                # 36 callers
sed -n 568105,568160p analysis/SWBFRONT.ELF-decomp.c        # BF1 class by flag bits
sed -n 568925,568951p analysis/SWBFRONT.ELF-decomp.c        # BF1 mode/prim from flags
sed -n 594259p analysis/SWBFRONT.ELF-decomp.c; sed -n 594346,594373p analysis/SWBFRONT.ELF-decomp.c
sed -n 572568p analysis/SWBFRONT.ELF-decomp.c; sed -n 572594,572602p analysis/SWBFRONT.ELF-decomp.c
sed -n 573363,573374p analysis/SWBFRONT.ELF-decomp.c; sed -n 573452,573454p analysis/SWBFRONT.ELF-decomp.c
sed -n 562030,562040p analysis/SWBFRONT.ELF-decomp.c        # TEX0 TCC/TFX (BF1)
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 559408 4   # Scroll vt: +0xc = 4b0c48
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 55e310 4   # Normal vt: +0xc = 4cf6f0
# Ghidra (3 runs, outputs kept in mtrl_scratch/): bf2 5901b0-590328, bf2 5909c0-590a80, bf1 4ac780-4ac800
```

## §8 Not closed
- **BF1 0x5100b (AFAIL FB_ONLY) without ABE** for mode-1 (0x02) world segments: read literally, texels with
  α<0x60 still write colour (not Z) — Endor's leaves would be solid cards, which the PS2 does not show. Either
  the VU microcode adds ABE/ALPHA for tested segments (word 0 of the state record is `ALPHA|VUflags`, VU-side
  meaning unread) or the munged RGB under α<0x60 is handled another way. [H] Needs a PCSX2 GS dump of END1.
- The VU-side prim bits 0x800800, 0x2000, 0x100000, 0x100 (BF2 `seg+0x04`, BF1 `seg+0x28`): meanings [H] only.
- Bucket ORDER (BF2 5/0xb/0xc/0xe/3/8/9/0x12 → `FUN_00585580`/`FUN_00584618`; BF1 `FUN_0049e2xx`): not traced.
- ps2BlurSegment draw (`FUN_004cf0a8`) and the BF2 bump path (`DAT_0060c650`, TNAM 2): not read; no shipped word selects Blur.
- Which world entity class calls slot +0xc for buildings/props in BF2 (the RTTI-less callers 305120/308861/313830):
  colour = obj+0x44 or white [V], class name [D].
- `DAT_007c5754` (global dim, 0.5 via `FUN_00456098`) — what event drives it [H pause/menu].
- Specmap words with 0x04 (e.g. 0x25/0x35), if any ship: ELF opaque vs v2's rule-2 BLEND — not counted here.
