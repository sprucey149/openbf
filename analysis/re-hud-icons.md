# LEAD VERIFICATION (Fable lead, 2026-09-09, session 17)
**Lead RE-READ, before this entered the Log — the three load-bearing hops:** (1) BF1 `FUN_004bbe90` sets ALPHA 0x44 / TEST 0x31001 for the bitmap records (B:580213–580427, the `DAT_006ff2xx` stores); (2) the `pal_` loader's CLUT alpha scale `(A & 0xff) * 0x81 >> 8` at B:571177–571201; (3) the bake's `alpha::keep_alpha` (`engine/crates/bake/src/alpha.rs:336–342`), which returns false for any non-sky texture that no MASK/BLEND draw names, so the sheet goes through `make_opaque` (`engine/crates/bake/src/map.rs:1010–1021`).
**VERDICT:** lead-verified. → queue item SM (the squad marker on, [REBAKE]: a texture no segment names keeps its disc alpha; the marker draws blended with no depth; ON by default). HOLD 4 is not changed by this report. The PS2's own placement of these sheets — a 2D D-pad command menu at HUD (−120,−50) — is recorded as the PROVISIONAL deviation, not built.
**Lead composition [D], recorded because neither report states it:** the ×129/256 CLUT scale closes `analysis/re-material-flags.md`'s header note ("the GS's 0x80-is-opaque blend arithmetic … NOT yet reconciled with a full-range CLUT") — disc 0xff → GS 0x80 = 1.0, so the port's full-range alpha as the BLEND factor is right. The ALPHA TEST is not: see queue item AT and `analysis/re-vu1-cull.md`. §5's open items (the second material layer, the additive mode, the HUD unit→pixel mapping) stay open. Everything below this header is the RE agent's report, verbatim.

# HUD icon compositing — how the PS2 draws `squad_cmd_*` and every HUD bitmap (Fable RE agent, s17, 2026-09-09)

Brief: `lead_tmp/s17/briefs/re_hud_icons.md`. Corpora: B: = `analysis/SWBFRONT.ELF-decomp.c`, D: = `analysis/SWBFRNT2.ELF-decomp.c`.
Headline: the PS2 draws every HUD bitmap by SOURCE-ALPHA BLEND with NO alpha test; the shape is the CLUT alpha the disc
carries (both icons have index 0 = alpha 0). The port's "alpha 255 on all 4,096 texels" is the BAKE's doing, not the disc's.
And the two sheets are a 2D D-pad command menu on the PS2, not world markers.

## 1. Load-bearing citations
1. **Squad menu group (BF1)**: ctor `FUN_002d81c8` B:264004–264069 (vtable 0x540b40; cross bitmap `FUN_004bba50` at +0x90 sized
   64×64 by `… loader
   `FUN_002d84a0` B:264101–264130 (binds `squad_cmd_cross` to +0x90, `squad_cmd_highlight` to the four, positions from
   `DAT_00518888`); select `FUN_002d8698` B:264180–264204 (selected alpha 0xff, others 0x7f, timer +0x120 = 1.0, visible);
   fade `FUN_002d8628` B:264134–264147 (timer −= dt·`DAT_00518844`=0.5/s, group alpha byte = timer·255, hidden at 0).
2. **Bitmap draw (BF1)** `FUN_004bbe90` B:580213–580402 = `RedBitmapElement` vtable 0x5596a0 slot +0x2c (ELF bytes at file
   0x3fa6a0; class set at B:580023): texture by FNV hash in table 0x6ef408 (B:580248), bind B:580283–580295, records
   `FUN_0039c1a8(ctx, DAT_006ff200|210|220|230, DAT_006ff240|0x54a5a0, 0x54a5a0, 0x54a5a0)` B:580302–580396, colour bytes halved
   B:580383–580386, 4 vertices `FUN_00398928(ctx,4,0x55dab0,…)`. Record init `FUN_004bc3a0` B:580403–580428; writer `FUN_0039c1a8`
   B:388492–388549 (opcode 0x404 + mask + up to 4 entries {u64 ALPHA, u16 PRIM, u32 TEST} in the qword order re-texture-wrap §1.2
   gives for BF2's 0x406). Parent×child colour in the group draw `FUN_004b7580` B:576996–577004.
3. **CLUT alpha at upload (BF1)** `FUN_004aebf8` B:571177–571201, per entry from the `pal_` loader `FUN_004aed68` B:571225–571318
   (`tex_` walker B:571474 dispatches `pal_`): A' = (A·0x81)>>8, then R|G<<8|B<<16|A'<<24. 32-bpp twin `FUN_004aecb0` B:571203–571223.
4. **TEX0 for the 2D bind**: `FUN_0049ea98` B:562808 → `FUN_0049e4b8` B:562572–562788 → `DAT_006ef3c8[0..1]` read by `FUN_004a8220`
   B:566590; the record is the one `analysis/re-material-flags.md` §2 "Texture function" reads (BF1 `FUN_0049c5c8` B:562033–562038:
   TCC=1, TFX=0 modulate).
5. **BF2 twins**: draw `FUN_005a88c8` D:741097–741286 (vtable 0x6509d0 slot +0x40; records via `FUN_004979a8` — the SAME 0x406
   material writer as the model path, re-texture-wrap §1.2 D:555841), init D:741292–741311, CLUT scale D:726290/726309, group
   `FUN_00391800` D:389300–389388 / `FUN_00391ac0` D:389390–389415 (hashes 0xa23c2305, 0x21add915), keys `FUN_00391730` D:389258–389284.
6. **Port**: `mungefs/src/texture.rs` 82–86 (alpha kept, full-range); `bake/src/map.rs` 954–1021 in `bake_content` (called by
   `bake_common` 391–392) + `bake/src/alpha.rs` 336–342 `keep_alpha` + `tier.rs` 232–240 `make_opaque`; marker
   `game/src/squad_vis.rs` 69–90 (`textured_masked`); `render/src/lib.rs` 323–335 (`textured_blended`, `DrawLayer::Blend`).
   **Corrected 2026-09-23 (cleanse):** the marker now draws `DrawLayer::Blend` (by the icon's alpha, no alpha
   test) and still depth-TESTS — it writes no depth (`game::squad_vis` header; queue SM).

## 2. The rule — how the icon becomes a shape
- [V] **GS state**: one material entry {ALPHA_1 0x44, PRIM 0x50 = TME|ABE, TEST_1 0x31001} for every textured bitmap
  (B:580408–580427: `6ff200`={0x44,0x50,0x31001}; D:741292–741311 byte-identical). ALPHA 0x44 → Cv = ((Cs−Cd)·As>>7)+Cd, i.e.
  source-over by fragment alpha. TEST 0x31001 → ATE=1 ATST=NEVER AFAIL=FB_ONLY, ZTE=1 ZTST=ALWAYS: no cutout, colour written, Z
  never tested or written. Not a colour key, not additive: 0x48 is the separate additive mode (`6ff220/230`, chosen only when
  element +0x64 == 1, B:580319–580330); the squad ctor leaves +0x64 = 0 (B:580020–580030), its users are B:53450/175916/361942–
  361995/519653 (not traced).
- [V] **The alpha**: As = At·Av>>7 (TFX modulate, TCC=1 — citation 4). At = GS CLUT alpha = (disc A·129)>>8 — the loader scales the
  disc's full-range 0..255 into the GS's 0..0x80 (0xff→0x80 = 1.0, 0x40→0x20, 0→0). Av = (colour.a+1)>>1 (B:580383–580386); colour =
  child byte × parent byte (B:576996–577004): selected highlight 0xff, the other three 0x7f, the whole menu × its fade timer.
- [V] **The disc icons are cutouts with soft edges** (pal_ decoded per citation 3's rule, §Repro): `squad_cmd_highlight` 64×64 4-bit,
  1,594/4,096 texels index 0 = {A 0, white}, 2,409 at A 0xff, 93 at 0x10..0xef; `squad_cmd_cross` 128×128 4-bit, 7,732 at A 0, 168
  at 0x40, 8,484 at 0xff; `hud_flag_icon`, `hud_icon_flash`, `player_icon`, `ambush_icon` are single-colour sheets whose 16 CLUT entries
  differ ONLY in alpha (0x00..0xff ramps) — their shape is nothing but alpha.
- [V] **The bake drops it**: mungefs emits the alpha (texture.rs 82–86) but `bake_content` sets `needs_alpha =
  keep_alpha(is_sky=false, …, world_needs)` (alpha.rs 336–342) = true only when a MASK/BLEND MODEL draw names the texture; no model names
  a HUD sheet → `make_opaque()` (tier.rs 235–240) before `write_ktx2`. Measured (mine): BF1 COMMON.LVL 253 `tex_`, 224 carry
  alpha < 255, **211 baked opaque**, 6 keep alpha (`com_icon_controlzone`, `item_powerup_health`, `lightning1/2`, `shell_cube`,
  `com_inv_col`), 7 have no ktx2; `common_bf2` `hud_flag_icon`/`player_icon`/`hud_objective_icon` alpha 255 on every texel.
  **Corrected 2026-09-23 (cleanse):** no longer true — queue SM (`f21c2db`) added the `unnamed` clause for the
  COMMON bakes, so the HUD sheets keep their disc alpha (`bake::alpha`, `bake::map`); see the FIXED note in §3.
- [D] **Port equivalent**: straight src-alpha / (1−src-alpha) blend, depth test and depth write OFF, texture alpha = the mungefs byte
  (the ×129/256 scale is the GS's 1.0 = 0x80 convention, so /255 is the same number), × the element alpha (1.0 / 0.5 / fade).
- [V] **Where**: a screen-space D-pad menu at HUD (−120, −50) (`DAT_0051883c/40`); cross 64×64, pivot (0.5,0.5), at the origin
  (`FUN_004b7df8` B:577311–577335); highlights 32×32 at (0,−16) rot π, (0,+16) rot 0, (+16,0) rot −π/2, (−16,0) rot +π/2 (`DAT_00518888`,
  matrix built by `FUN_004b7e48` B:577337–577365 — one arm sprite rotated four ways); `gamefont_tiny` labels at (0,−25)/(0,+25)/(+25,0)/
  (−25,0) align (2,1)/(2,0)/(0,2)/(1,2) (`DAT_00518848`), text from `DAT_0072eee8[class·4+i]`; shown by `FUN_002d8698(group, class,
  selected)` from the pad handler B:181630. BF2 tables byte-identical (`DAT_005ff30c/318/358`), label keys
  `game.squadcommand.{field,driver,passenger,gunner}.{up,down,right,left}` (ptr tables 0x5ff388/0x5ff398). The port's world marker over
  bots is therefore PROVISIONAL (ours) in placement; this doc fixes only its compositing.
- [V] **BF2 is the same path**, and asks for the SAME sheets: 0xa23c2305 / 0x21add915 = the ELF FNV-1a (`FUN_00444b30` B:504384–504400,
  `|0x20` on every byte, `_`→0x7f) of `squad_cmd_cross` / `squad_cmd_highlight` (recomputed). **No BF2 PS2 LVL carries either name**
  (grep of every file under `extracted/bf2/lvl`; every NUL-terminated string on the disc hashed against both constants: no hit), so
  `FUN_00539150(0x7c59d8,0x800,hash)` returns null and the draw takes the fallback branch (D:741112–741140: `FUN_00584b10(hash,0,0)`,
  records `DAT_0063c690`). [H] what a BF2 PS2 shows there (default texture or nothing) — capture. BF2's own named HUD icons:
  `com_icon_*` (36: faction/flag/controlzone/ammo/health/build/swap holograms), `hud_objective_icon{,_circle,_hidden,_offscreen}`,
  `hud_dest_icon`, `hud_reinforcement_icon`, `hud_vehicle_seatingchart_{ai,empty,player1}_icon`, `troop_icon`, `turret_icon`,
  `ambush_icon`, `blank_icon`, `player_icon`, `hud_flag_icon` (INGAME.LVL); `hud_icon_flash`, `{all,cis,imp,rep}_icon`, `lobby_icons` (COMMON.LVL).

## 3. Draft RE_LOG.md entry (≤ 15 lines)
### HUD bitmaps composite by CLUT alpha — source-alpha blend, no alpha test, and the loader scales CLUT alpha ×129/256 (2026-09-09, Fable session 17) → `engine/artifacts/lead_tmp/s17/re/hud_icons.md`
Pointer entry; the report is the record. Raised by phase-d's squad marker drawing a tan square (`20260909_0035_phase-d.md` l.66–70).
- [V] Every HUD icon on both discs is a `RedBitmapElement`; its draw emits ONE material entry {ALPHA 0x44, PRIM TME|ABE, TEST 0x31001}
  (BF1 `FUN_004bbe90` 580213–580402 + init 580408–580427; BF2 `FUN_005a88c8` 741097–741286 + 741292–741311): source-over by As, no
  cutout, no Z. As = CLUT alpha × ((colour.a+1)>>1) >> 7, TFX modulate.
- [V] The `pal_` loader scales every CLUT alpha (A·0x81)>>8 at upload (BF1 `FUN_004aebf8` 571177–571201, BF2 726290/726309) — disc
  0xff IS the GS's 1.0. Lead to re-read before adopting: this reconciles `re-material-flags.md`'s open "0x80 vs full-range" item.
- [V] The disc icons ARE cutouts (`squad_cmd_highlight` 1,594/4,096 texels alpha 0); the BAKE forced them opaque — `bake_content`
  (map.rs 958–1021) kept alpha only for textures a MASK/BLEND model draw names; 211 of BF1 COMMON.LVL's 224 alpha-bearing sheets
  baked opaque, `common_bf2` likewise. The loop's "alpha 255 everywhere" measured the bake, not the disc.
  **FIXED 2026-09-17 by queue item SM (`f21c2db`) — for the COMMON bakes only.** `keep_alpha` gained an `unnamed` input, so a
  sheet that no top-level and no mode-layer segment names keeps its disc alpha when the content is `Content::Common`; the
  shipped `common_bf1/textures/squad_cmd_highlight.ktx2` now carries **1,594 of 4,096 texels at alpha 0** and
  `squad_cmd_cross.ktx2` **7,732 of 16,384** — this section's own counts, re-read off the shipped KTX2 by the landing session.
  288 baked files moved (201 `common_bf1` + 86 `common_bf2` + `common_bf2/textures/map_mask.tier2.ktx2`) and no map file did:
  a MAP sheet still bakes opaque, because on the water maps the freed alpha changes the water pass's look and the PS2 fact
  that would settle it is unread (`re-water.md` §8, HOLD 21). That half is queue **WA**.
  **Corrected 2026-09-23 (cleanse):** WA landed 2026-09-23 (`19975d0`) — the map's own water sheets keep their
  disc alpha; other unnamed MAP sheets (terrain, effects) still bake opaque.
- [V] `squad_cmd_cross`/`_highlight` are the 2D D-pad command menu (HUD (−120,−50); cross 64², four 32² arms at ±16 rotated
  0/π/±π/2, labels at ±25; selected 0xff vs 0x7f; 0.5/s fade), same tables in BF2, which asks for the same sheets by FNV hash but
  ships neither on the PS2 disc (fallback branch; what it shows is [H]).
- Port: queue item SM — keep the alpha of every sheet no model names, blend the marker (`textured_blended`, depth off), flag on.

## 4. Queue item (QUEUE.md shape)
SM. [TODO — after PD's FINAL commit lands (PD owns `game/src/main.rs`, `game/src/squad_vis.rs`); `bake/src/map.rs` is shared with
nothing IN PROGRESS] **the squad marker on** [REBAKE] [GOLDENS only if the sweep moves one — none expected]. RE: `lead_tmp/s17/re/hud_icons.md`
(lead to verify §1 citations 2, 3, 6 before the bake rule is adopted — it reverses design D3's corner for textures NO model names).
Build: (1) bake — `bake_content` (map.rs 958–1021): a texture that NO segment names (top `None` AND layer `None`) keeps its disc alpha
(hud_icons.md §2: the PS2 draws every such sheet as a `RedBitmapElement` blended by that alpha); MASK/BLEND-driven textures unchanged;
report the count per bake (expect 211 in common_bf1). (2) `squad_vis.rs` 84: `DrawCmd::textured_blended` in place of `textured_masked`,
depth test and write OFF for this draw as TEST 0x31001 has it (if `DrawLayer::Blend` writes depth, add the no-depth variant in
`render/src/lib.rs` beside 331); element alpha 1.0. (3) `main.rs` 395: the marker is ON unless `OPENBF_SQUAD_MARKER=0`; doc-comment the
PS2 placement (§2 "Where") as the recorded deviation. (4) tests: data-gated `bake`/`mungefs` test — `squad_cmd_highlight` decodes with
1,594 alpha-0 texels and the written KTX2 keeps them; unit test — the marker's DrawCmd is `DrawLayer::Blend`.
GATE: `cargo test -p bake -p mungefs -p game` green; `cmp_bake` diff = alpha-bearing files only, count reported; D14 headless capture
(TAT1 conquest seed 42, `--hud --capture-series`, marker on): `walk_02000.png` shows the ring-shaped cyan/white highlight over the six
bots with the ground visible through its alpha-0 texels — no tan square (lead views); sim hash unchanged `ebb933882466ff06`;
`flock loop/ci.lock scripts/gpu_tests.sh` EXIT 0 with `tat1_hud` unmoved (`golden_hud` draws the egui HUD only, no disc sheet).
Files: `engine/crates/bake/src/map.rs`, `engine/crates/bake/src/alpha.rs` (only if `keep_alpha` gains the "unreferenced" input),
`engine/crates/game/src/squad_vis.rs`, `engine/crates/game/src/main.rs`, `engine/crates/render/src/lib.rs` (only for the no-depth blend
variant), one data-gated test file under `engine/crates/bake/tests/` or `mungefs/src/texture.rs` tests.

## 5. Still open
- [H] The VU1 side of opcode 0x404 (BF1) / 0x406 (BF2) for a bitmap: the second entry {ALPHA 0x89 = Cd·As, PRIM 0x58, TEST 0x51001}
  (`6ff240`) is passed whenever a texture is bound (B:580328) — if the microcode draws it as a second layer the icon would darken the
  frame by its alpha; a PCSX2 capture of the D-pad menu settles it (and the BF2 fallback texture). The 0x54a5a0 entries are "off"
  layers (ALPHA flags 0x300, PRIM 0, TEST 0x30000).
- The additive mode (+0x64 = 1, ALPHA 0x48) — which HUD elements use it (B:53450, 175916, 361942–361995, 519653 set it) — untraced;
  the port's HUD draws no disc sheet yet, so nothing depends on it today.
- HUD unit → pixel mapping: the sprite routine maps 0..1 to the viewport (`FUN_002ef540` B:278002–278099 for the HUD-code sprites,
  ALPHA 0x44 / TEST 0x31001 / halved colour — the same compositing); the element coordinates' virtual scale is [H].
- The CLUT ×129/256 scale is the one `pal_` loader for every `tex_` (B:571474), so it applies to model textures too — [D] that this
  closes re-material-flags §8's "0x80 vs full-range" gap; the lead re-reads B:571177–571201 before the RE_LOG says so.

## Repro (read-only)
```
sed -n 264004,264069p analysis/SWBFRONT.ELF-decomp.c; sed -n 264101,264204p analysis/SWBFRONT.ELF-decomp.c   # menu ctor/loader/select/fade
sed -n 580213,580428p analysis/SWBFRONT.ELF-decomp.c; sed -n 388492,388549p analysis/SWBFRONT.ELF-decomp.c   # bitmap draw, records, writer
sed -n 571177,571318p analysis/SWBFRONT.ELF-decomp.c                                                        # CLUT converter + pal_ loader
sed -n 741097,741311p analysis/SWBFRNT2.ELF-decomp.c; sed -n 389258,389415p analysis/SWBFRNT2.ELF-decomp.c   # BF2 twins
python3 -c "import struct;b=open('extracted/bf1/SWBFRONT.ELF','rb').read();o=lambda v:v-0x15f000;print([struct.unpack('<f',b[o(0x518888+i*4):o(0x518888+i*4)+4])[0] for i in range(12)])"
# icon pal_ decode: tex_ at 0x684d4 (highlight) / 0x677f8 (cross) in extracted/bf1/lvl/DATA/_LVL_PS2/COMMON/ENG/COMMON.LVL — RLE per mungefs/src/rle.rs,
# entries [A,B,G,R]; baked alpha: KTX2 level 0 at the offset in the level index (bytes 80..104), every 4th byte.
grep -rl -a -i squad_cmd extracted/bf2/lvl   # empty
```
