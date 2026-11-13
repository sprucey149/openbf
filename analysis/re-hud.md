# OPUS RE (2026-09-30) - [O] UNAUDITED
# BF2's HUD: where its art and layout live on the disc, and how the PS2 draws them (card BHUD)

> **Correction (2026-10-13, card CPBAR, `analysis/re-capture-display.md`):** "the whole BF2 HUD is data on the disc"
> holds for the layout tree only. The capture icon (section 3's CP capture row, corrected) is drawn by a code
> object, one of ten code-drawn HUD pieces the pause menu switches on and off together (B2:385840-385850); the
> other nine are not read. The layout's `commandPost` ring is dead data on the PS2.

Corpora: D: = `analysis/SWBFRNT2.ELF-decomp.c` (BF2), B: = `analysis/SWBFRONT.ELF-decomp.c` (BF1). RAM = PCSX2 save state
`engine/artifacts/pcsx2/ref/states/bf2_hot1_cp1_cp6_postA_spawn.p2s` (`eeMemory.bin`, VA = offset). Reference frame =
`engine/artifacts/pcsx2/ref/bf2_hot1/cp1_cp6_postA_v1.png` (640×480 capture, the HUD visible). Tools (all in
`engine/artifacts/lead_tmp/re/bhud/`): `huddump.py` (dumps every `hud_` chunk as a named tree; `--hashes` lists unnamed
hashes), `names.txt` (the recovered names), `brute.c` (FNV brute force), `dump_named.txt` (the full dump, 14,245 lines).
Tags: [V] read (lines cited), [D] derived (from what), [H] hypothesis, [M] measured on the PS2 frame / RAM.
Hops: (1) chunk census of the BF2 LVLs → five `hud_` config chunks in INGAME.LVL; (2) FNV name recovery (`brute.c`,
dictionary from the BF2 Lua sources + context) → `huddump.py` tree; (3) the ELF's registry `FUN_003b8040` and the
property parsers per type; (4) the draw side — the Red scene `FUN_005a3d40`, the bitmap and model draws, the HUD camera
constants in RAM; (5) the publishers `FUN_003b61e0` → `FUN_003b59a0` (teams), `FUN_003b3398` (weapons); (6) the PS2 frame
(HOT1 postA v1) as a zero-parameter check of the projection; (7) `re_check.py` fn/absent/word/writers (§5); (8) one Opus
verifier, 21 claims: 20 SUPPORTED, 1 PARTLY (C15 wording, fixed) — §6; a second Opus verifier on the weapon-mesh key
(PARTLY → the corrected rule in §3) and the NameMesh fallback (SUPPORTED). Load-bearing claims carry [O].

## Headline
- **The whole BF2 HUD is data on the disc.** INGAME.LVL carries five `hud_` config chunks — `hudtransforms`,
  `1playerhud`, `2playerhud`, `3playerhud`, `4playerhud` — the munged form of the PC tools' `.hud` files (same
  DATA/SCOP format as `fx__`/`sky_`, `mungefs::config`). Every element (type, name, position, size, texture or mesh,
  font, colour, alpha, z-order, fades, and the game event it listens to) is in them. 145 of the 156 distinct property
  hashes are named by FNV (11 unnamed, all minor). The bake can parse them as they are; nothing needs hand-typing.
- **The art:** textures in INGAME.LVL `tex_` (plus five in BF2 COMMON.LVL and `white` in CORE.LVL), 3D HUD meshes in
  INGAME.LVL `modl` (reticle, bar frames, backdrops, vehicle-weapon icons), the weapon icons are the WEAPONS' OWN
  MODELS (side LVLs), the four bitmap fonts in CORE.LVL `font` chunks, the minimap image is the map LVL's `<world>_map`
  texture masked by `map_mask`. The port already bakes the INGAME textures and meshes; not the layout, the fonts, the
  five COMMON sheets or the side-LVL `hud_*` meshes.
- **How the PS2 draws it:** a small retained scene ("Red" elements) under ONE perspective camera per viewport — 60°
  horizontal FOV, a 4:3 frustum whatever the 512×448 buffer. The HUD root scales and pushes everything by fixed factors
  (1.23, 1.3, 1.24), so a `"Viewport"` fraction fx, fy lands at display fraction x = −0.0059 + 1.0139·fx,
  y = −0.0242 + 1.0484·fy; a native pixel / font texel is H/378.7 × H/427.3; a 3D HUD mesh unit (after `Scale`) spans
  0.2129 H × 0.1887 H (H = display height). Checked against the PS2 frame with no fitted parameter (±1 px on bars and
  counts). Z-order 0 is in front. Every element fades in / holds / sustains / fades out on its enable/disable events.

## 1. Where it lives on the disc
### 1.1 The layout — five `hud_` chunks in BF2 INGAME.LVL [V][O]
Top-level chunk census of `extracted/bf2/lvl/DATA/_LVL_PS2/INGAME.LVL` (walk of the `ucfb` children): `hud_` × 5 at file
offsets 0x2ddbd8 (6,720 B, `hudtransforms`), 0x2df620 (90,924 B, `1playerhud`), 0x2f5954 (179,380 B, `2playerhud`),
0x321610 (328 B, `3playerhud` — one `Bitmap("blackmask")` for the unused quarter), 0x321760 (194,016 B, `4playerhud`).
Each is `NAME`(hash) + a run of `DATA`/`SCOP` records (`mungefs/src/config.rs` header: `[u32 fnv(name)][u8 argc][argc
slots][u32 blobLen][blob]`, a SCOP = the children of the DATA before it). The PC mod tools list the same five names as
`config` in `ingame.req` (`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/SourceCode/Common/ingame.req`, last
REQN). No BF1 LVL carries a `hud_` chunk (census of every `extracted/bf1/lvl/**/*.LVL`): BF1's HUD is built in code.
- **Names** [V by hash]: FNV-1a over `c|0x20` (`mungefs::fnv1a_lowercase`) of the names in `names.txt` equals the stored
  hashes. Recovered by dictionary + 2–3-word brute force (`brute.c`) + "the key equals its own string argument" (26 Map /
  Target sub-element keys, e.g. `PlayerFOVSmall("PlayerFOVSmall")`). Collision risk: a 32-bit hash over ~15 M candidate
  strings vs 156 targets expects < 0.6 false hits; every hit also reads right in context (e.g. `Alpha(0.6)`,
  `ZOrder(253)`, `EventEnable("player1.health")`). Unnamed (11): 2b76908f, 2deba703 (BarBitmap flash-on-rise/-fall
  times, §2.2), 7a4e6e71 (a Text flag, +0x1c8 bit 33), 233d39ec/0d533bd4 (Map mode events), 92c1cee6/9d34215e (Target
  reset events), 6703febc (Target flag), 86e6ef58 (an element type: the hero bar's pulse bar), 09771c74 (MultiLineText
  flag), 4ee68dbe (hudtransforms' `"player%"` pattern).
- **Element types** — the registry `FUN_003b8040` (D:412764–412830) creates one factory per type [V]: Group 0x5fb91e8c,
  Text (`FUN_003adba0`, 0xbde64e3e), Bitmap 0x46544626, `FUN_003a19e0` (?), Model3D (`FUN_003a9790`, 0x1eccdefd, D:403150),
  Map (`FUN_003a2d20`, 0xdfa2efb1), Target (`FUN_003acae8`, 0x32608848), `FUN_003abae0` (?), TransformNumberColor
  0x0d34920d, (0xf3c47270 — a fourth transform, unused by the data), TransformNameMesh 0x67c3c715, TransformNumberVector3
  0x2ef8f006, ObjectiveList (`FUN_003b91f8`, 0x6fa0d380), BorderedBox (`FUN_0039c550`, 0xb3a05a43), MultiLineText
  (`FUN_003aa398`, 0xd53e50d7), BarBitmap (`FUN_0039f4e0`, 0x3bd46d91 — a Bitmap subclass, D:397122–397125), the pulse
  bar (`FUN_003b9c68`, 0x86e6ef58), BarSegmented (`FUN_003a0240`, 0x4b4bf31a), `FUN_003aec50` (?), Viewport
  (`FUN_003bc348`, 0xe4abbac3 — the `hudtransforms` container), 0x0e0d9594 (?), FileInfo 0xd4e0c797. Absent from BF1:
  `hashloc.py` finds 0x46544626 (Bitmap) and 0x1eccdefd (Model3D) only in D: (B1's three 0x5fb91e8c hits are its shell).

### 1.2 The art [V unless marked]
| what | where on the disc | baked by the port today |
|---|---|---|
| HUD sheets (fills, icons, markers, `map_mask`, `hud_white`, …) | INGAME.LVL `tex_` (134) | yes, `common_bf2/textures/` (alpha kept, queue SM) |
| `hud_icon_flash`, `border_3_pieces`, `opaque_black`, `btn_directional_pad*`, `stats_kills` | BF2 COMMON.LVL `tex_` (grep) | no |
| `white` (segment bitmaps) | BF2 CORE.LVL `tex_` | no |
| `hud_target_hint_offscreen` | nowhere on the disc (named only in the config) → the PS2's missing-texture path, as `re-hud-icons.md` §2 found for `squad_cmd_*` | — |
| 3D HUD meshes: `hud_main_reticule{,_bullseye,_outline,_hit,_criticalhit,_rocketlockon_active}`, `hud_energybar_frame_{front,back}`, `hud_jetpackbar_frame_{front,back}`, `hud_weapon{,2}_backdrop`, `hud_infinite_ammo_icon`, `hud_map_fov`, `hud_large_map_fov`, `hud_arrow`, `hud_vehicle_*`, `hud_force_*`, … (49 of the 101 mesh names the 1-player HUD + transforms use) | INGAME.LVL `modl` | yes, `common_bf2/models/*.glb` (49/49) |
| the other 52 names: the weapons' own models (`rep_weap_inf_rifle`, `imp_weap_inf_pistol`, …) and side-only HUD meshes (`hud_cis_trishot` CIS.LVL, `hud_wampa` SNW.LVL, `hud_stealth` ALL.LVL, `hud_imp_flamethrower` IMP.LVL) | side LVLs | weapon GLBs yes (soldier bake); the side `hud_*` meshes no |
| fonts `gamefont_{tiny,small,medium,large}` | BF2 CORE.LVL `font` × 4 | no |
| minimap image | the map LVL's `tex_` named `sprintf("%s_map", world)` — `hoth_map` (HOT1), `uta1_map` (UTA1), `myg1_map` (MYG1) | yes, per map (`bf2_hot1/textures/hoth_map.ktx2`); BF1 maps carry their own (`naboo1_map`) |

- **Fonts** [V][O]: `font` = `NAME`, `HEAD` (6 B: u16 glyph count 226 = 0xe2, u8 pages, u8 line height: large 1 page/22 px,
  medium 3/18, small 3/18, tiny 2/15), `FTEX` (per page `NAME` + a normal `tex_`, 128² 4-bit; large one 256² sheet),
  `FBOD` (226 × 24 B: u16 char code, u8 page, u8 advance, u8 x0, u8 x1, u8 y0, u8 y1 — the glyph box in texels — then f32
  u0, u1, v0, v1). E.g. medium `'!'` = page 0, advance 5, box 0..4 × 0..16, uv 0.1328–0.1641 × 0.0078–0.1328 (= 4×16
  texels on a 128 sheet). Medium and small carry identical metrics.
- **Minimap name** [V][O]: `FUN_003a7af0` D:402114 `FUN_0051fbf8(buf, 0x100, 0x62fc20 "%s_map", 0x825da0)` then binds it to
  three map views (D:402113–402120) with the mask `0x62fc28` = `"map_mask"`. `0x825da0` is written by `FUN_004272b0`
  D:483238 (`strncpy`, 0x40); RAM at 0x825da0 = `"hoth"` in the HOT1 state, `"uta1"` in the UTA1 state [M].

## 2. The element model (BF2 ELF)
### 2.1 Common properties — `FUN_0039e1a0` D:396176–396460 [V][O]
| name (hash) | line | effect |
|---|---|---|
| Position (0x934f4e0a) | `FUN_0039d258` D:395502 | (x, y, z) into the element matrix translation; 4th arg = coordinate mode from the table at 0x5ff850 {Pixels, Screen, Viewport, Frame} (a non-string 4th arg: nonzero → Viewport) |
| Rotation (0x21ac415f) | `FUN_0039d358`→`FUN_0039d3f8` D:395599 | Euler degrees ×0.017453294, X then Y then Z, translation kept |
| Scale (0x82971c71) | `FUN_0039d3a0`→`FUN_0039d4d8` | per-axis scale vector |
| Viewport (0xe4abbac3) | D:396373 | which player viewport, clamped ≤ 4 (`FUN_0039d9b8`) |
| Color (0x3d7e6258) | D:396276 | RGB clamped 0..255, alpha byte 0xff → render element +0x40 and the target colour +0xb0 |
| ColorChange (0x3c656d30) | D:396252 | the pulse colour +0xb4 |
| Alpha (0x5d8b6dab) | D:396316 | +0x7c (default 1.0, `FUN_0039d638` D:395827) |
| ZOrder (0xec9f263f) | D:396410 | byte 0..255 → `FUN_005a3368` |
| BlendMode (0xfa784eab) | D:396401 | table 0x5ff848 {Alpha, Additive} → render vtable +0x14 |
| FadeInTime / FadeHoldTime / FadeSustainTime / FadeOutTime (0x5083ecd9 / 0x8e3e2923 / 0xff48a8f9 / 0x798dc484) | D:396309 / 396348 / 396423 / 396323 | fader slots 1 / 2 / 3 / 4 (`FUN_003aff58`) |
| ColorChangeRate (0xb4660872) | D:396353 | +0xb8 (default 0.3 = 0x3e99999a, D:395842) |
| ColorPulseRate (0xe459101b) | D:396392 | +0xbc and the timer +0xc0 (default FLT_MAX, D:395843–395844) |
| UseChangeColor (0x849604eb) | D:396221 | +200 bit 32 |
| EventEnable / EventDisable / EventColor / EventPulseRate (0xd23096a8 / 0x7b5f76b1 / 0x49f429ae / 0xccd945ca) | D:396388 / 396328 / 396230 / 396337 | listeners at +8 / +0x18 / +0x38 / +0x48; handlers (ctor D:395846–395850): enable → show (`FUN_0039ef58`), disable → hide (`FUN_0039efa8`), colour → the target colour +0xb0 (`FUN_0039f070`, event type 7), pulse rate → +0xbc (`FUN_0039f0d0`) |

- **Coordinates** [V] (the size labels [D]): `FUN_0039d0e8` D:395418: mode 0 Pixels = value, 1 Screen = value × screen size, 2 Viewport = value ×
  viewport size, 3 Frame = value × the parent frame size. Screen size DAT_007d5f70/74 = 512 × 448 [M, RAM]; safe-area
  globals 0.05 / 0.9 at 0x7d5f78–84 [M] (their use not traced). Positions nest: a child's Position is relative to
  its parent group [D: the frame check in §3 only adds up that way].
- **Z-order** [V][O]: `FUN_005a3368` re-links the element into its scene list before the first node whose z ≤ its own, so the
  list runs from high z to low; the scene draw `FUN_005a3d40` walks it from the head → **255 is drawn first (back), 0
  last (front)**. Default is **128**, stored at render +0x48 by `FUN_005a30b0` / `FUN_005a3178`
  (B2:737260–737261 and 737300–737301); the prior default-zero hypothesis covered the jet fuel fill with its
  opaque frame. The data agrees: `player1health_fill` ZOrder 2 over `player1health_empty` ZOrder 3.
- **Fader** [V][O] (`FUN_003afca8` D:406942 ctor: all times 0 but Hold = 0x7f7fffff; advance `FUN_003afe18` D:407029; alpha
  `FUN_003afe90` D:407072; show `FUN_003afd18` D:406970; hide `FUN_003afda0` D:407005): states 0 hidden (α 0) → 1 in (α =
  t/FadeIn) → 2 hold (α 1, FadeHoldTime, default forever) → 3 sustain (α 1, FadeSustainTime) → 4 out (α = 1 − t/FadeOut)
  → 0 (state 4 already gives α 0 once t ≥ FadeOut). A zero time skips its state. Show: 0→1, 2 restarts the hold, 3→2, 4→1 at the mirrored alpha. Hide: 1→4 at the
  mirrored alpha, 2→3.
- **Colour and alpha each frame** [V] `FUN_0039da30` D:395934 (only while the render element's visible bit, +0x48 bit 8, is set): the drawn colour eases toward Color by min(dt /
  ColorChangeRate, 1) per frame; the pulse timer counts down by dt and at ≤ 0 snaps the drawn colour to ColorChange and
  restarts at ColorPulseRate; drawn alpha byte = fader α × Alpha × 255.

### 2.2 Bitmap / BarBitmap / BarSegmented [V][O]
- Bitmap `FUN_003a1440` D:398260: `Bitmap("tex")` binds the texture by name (render vtable +0x50); EventBitmap
  (0x5cd8e364) swaps it at run time; TexCoords (0x1ffad2e0) = u0, v0, u1, v1; BitmapRect (0x625fa794) = w, h, horizontal
  align {Left, Center, Right} (0x5ff870), vertical align {Top, Center, Bottom} (0x5ff880), mode (0x5ff850) → render vtable
  +0x4c; BitmapStyle (0xa28f0eeb) {Normal, Shadow} (0x5ff868) → render +0x60.
- Draw `FUN_005a88c8` D:741097–741286: the compositing of `re-hud-icons.md` §2 (source-over by the texture × colour alpha,
  no Z) — plus **Shadow** (D:741207–741233): first the same quad translated (+1, +1) with the global shadow colour
  DAT_0076c220 (RAM 0x000000 black [M]; only writer `FUN_0057ca38` D:712412 = 0) at half the element alpha.
- BarBitmap `FUN_0039fe08` D:397387 + update `FUN_0039f978` D:397236: a value 0..1 (Value 0x425ed3ca initial, EventValue
  0x8500add4, helper `FUN_0039f368` D:397048) shortens the rect from the left, width × value, and — unless ScaleTexture
  (0x79b5f340) is 0 — the u range with it (u1 = u0 + Δu × value); a second copy flashes over the changed span
  (FlashyScale 0xf5989fb2 default 0.2; 2b76908f / 2deba703 = its time when the value rises / falls, +0x364 / +0x368 —
  used only while +0x370 bit 35, refreshed from +0x138, is set and the chosen time exceeds 1e-4).
- BarSegmented `FUN_003a0c08` D:397977: AngleStart / AngleEnd (degrees → radians), NumSegments, Circular (0x026723e0),
  the same value helper; its `segment("Segment")` child is the bitmap repeated around the arc inside `Rect`.

### 2.3 Text [V] — `FUN_003ae050` D:405843 ff.
TextFont (0x90ae147f) by name; `Text("key")` localises the key (`FUN_005ac980`) else prints it raw; EventText (0x7e1311a8)
/ EventNumber (0x3fc8df9e) feed it; TextAlignment (0xc2200723) = horizontal {Left, Right, Center} (0x5ff9c8) + vertical
{Top, Bottom, Center, Baseline} (0x5ff9d8); TextStyle (0x15746cb3) {Normal, Shadow, Selected} (0x5ff9e8); TextBox
(0x5bace35b) = w, h × the screen size (… TextBreak (0x37d0f255) table
0x5ff9b0 {None, Word, No Reformat, Word Reformat}; TextScale (0x133c15dc); TextCharacterSpacing (0xf30abeaa);
NumberToTime (0xce8ad9c0, e.g. `"%m:%02s"`); FloatFormat (0xd9e54312).
- [V] A Text without TextFont draws in **`gamefont_tiny`**: the Text ctor (D:405704) sets its text object's font to
  `DAT_0060c794`; `re_check.py word b2 0x60c790 3` -> 0x60c790 0x8f4b7122, **0x60c794 0x46f060d5**, 0x60c798
  0x548190d5, and `manifest::hud::locl_key("gamefont_tiny")` = 0x46f060d5 (0x8f4b7122 = `gamefont_small`). The
  domination countdown's "Victory In" / "Defeat In" labels have no TextFont, so they print in the tiny font.
- [V] An INFINITE number prints no digits (card HUDW, 2026-10-05; `re_check fn`: B2:406187 and 406209 are in
  `FUN_003ae6c0`, the Text's event handler; no verifier agent - a bounded read inside a build card): an int event equal
  to -1 (type 3) or 0x7fffffff (type 2), or a float FLT_MAX (type 4), takes the string at 0x630580 = "" when the
  element's +0x1c8 bit 33 is clear and 0x630588 = "--" when set (`re_check word b2 0x630580 12`: 0, 0, 0x2d2d, 0,
  "%d", …); the bit is property 0x7a4e6e71 (`FUN_003ae050` B2:405841-405846). Both ammo Texts carry `#7a4e6e71(0)`:
  an infinite weapon shows the `hud_infinite_ammo_icon` ring and NO count. [M] PS2 frames
  `engine/artifacts/lead_tmp/hudw/ps2/` (BF2 KAM1: the clone's pistol, the SBD's tri-shot; the count returns with the
  counted weapon and the ring goes).
- Glyph size: see §2.4 (a font texel is one element-local unit = one pixel of the 512×448 frame).

### 2.4 Model3D and the HUD projection
- [V] `FUN_003a9e10` D:403404: Mesh (0xa100bebc) = a model by name (the model table 0x76c310, `FUN_0059fc30`);
  EventMesh (0xf845adf2) swaps it; Lighting (0x827eb1c9) = render +0x70 bit 0 (default 1, `FUN_0059fbd0`); MeshInfo
  (0x4b426ca0, `FUN_003a99e8` D:403231) = per-mesh-name Position/Rotation/Scale that REPLACE the element's own transform
  when that mesh is shown (`FUN_003a9db0`); +0x60 default 0.01 (0x3c23d70a).
- [V] Draw `FUN_0059fe60` (vtable 0x650970 = `21RedModel3DElement`, slot +0x40): `FUN_0059fdb0` negates the matrix's Y
  row and multiplies its TRANSLATION row by 0.01 × (DAT_0060c774 ? DAT_0060c7b8 = 1.24 : 1) (eedis: `VMULx.xyz` at
  0x59fddc on row 1 and at 0x59fe14 on row 3), then renders the model with that matrix.
- [V] Scene draw `FUN_005a3d40` D:737783 with `FUN_005a3bf0`: DAT_007d6208 = √3 (`FUN_0052bc10(3.0)`), 620c = √3·2/3,
  6210 = √3/2 (`FUN_005a45d0` D:738186–738188); W, H = 512, 448 (DAT_007d5f70/74, RAM); plane depth D = W/1.1547
  (a local); per scene ROOT r (the list walked at D:737943–737948; each node's draw, slot +0x40, gets root-matrix × node-matrix,
  D:737989–738035) the anchor (ax, ay) = (r+0x80, r+0x84); a root WITHOUT r+0x88 bit 1 sits at (W·(ax−½), −0.75W·(ay−½),
  −D) with matrix diag(b8, b4, 1); a root WITH bit 1 (also latched into DAT_0060c774 by `FUN_0059fd80`, D:737961–737962)
  at (1.23·1.02·W·(ax−½), 1.3·1.0·(−0.75W)·(ay−½), −1.24·D) with matrix diag(1.23·b8, 1.3·b4, 1) (D:737965–737986).
  DAT_0060c7b0..c0 = 1.02, 1.0, 1.24, 1.23, 1.3 (ELF .data words; no text-visible writer); b8 = DAT_007c56e8 × √3·2/3 /
  (√3/2) = 0.7666 × 4/3 = 1.0221 (DAT_007c56e8 only writer `FUN_00580b08` D:715827 = 0x3f443fe6); b4 = −0.75·W/H = −0.857.
- [V] The HUD's roots: `FUN_003b8040` builds two arrays of five roots (DAT_005ffa60, DAT_005ffa64, D:412832–412847) and
  sets entry 0 of each to z 0, anchor (0, 0) and **+0x88 bit 1** (D:412848–412862); an element joins the first array unless
  its +200 bit 35 is set (`FUN_0039e9a8` D:396584–396591). Bit 35 is cleared by the element ctor (D:395845), copied from a
  template by `FUN_0039de48` (D:396075–396076) and SET only by property 0xca0b9ccb (D:396363) — a hash no BF2 LVL contains
  (verifier: 165 LVLs searched), so every HUD element joins the first array. [H] Viewport(1) = entry 0 (the split-screen roots are placed elsewhere, D:385852, D:386102).
- [D][O] The camera: horizontal half-angle atan(1/√3) = 30°, vertical tan = 0.375W/D = 0.433 (23.4°): a 4:3 frustum
  whatever the 512×448 buffer. With the flagged root and anchor (0, 0), a viewport fraction lands at display fraction
  **x = −0.0059 + 1.0139·fx, y = −0.0242 + 1.0484·fy** (the 1-player HUD overfills the 4:3 display by 1.4 % × 4.8 %); an
  element-local unit (a native pixel: BitmapRect × 512/448, a glyph texel) is 1.0139/512 of the display width
  (= H/378.7 on 4:3) by 1.0484/448 of its height (= H/427.3); a Model3D mesh unit after Scale, drawn at depth
  1.24·D·0.0124, is **0.1597 of the display width (0.2129 H) by 0.1887 H** — 1.128× wider than its true proportions.
- [M][O] Checked with ZERO fitted parameters on the frame: health fill predicted capture x 38.3–169.4, y 404.8–412.2 →
  measured 39–169, rows 406–413; friendly / enemy count centres predicted 173.5 / 467.0 → measured 172.5 / 466.5;
  "147" (`gamefont_medium` '1' '4' '7', advances 7 + 12 + 11, ink 12 texels tall) predicted 38.0 × 13.5 → measured 38 × 14;
  the reticle outline mesh (2.885 × 2.987 units × Scale 0.26) predicted 76.2 × 73.4 → measured 76 × 70. The unflagged
  alternative predicts the reticle at 96 × 83 and the health fill 2 px left / 18 px higher — rejected.

### 2.5 Transforms [V]
TransformNameMesh `FUN_003badc8` / apply `FUN_003bafa8` D:414723–414850: NameMesh(in, out) pairs, sorted lookup; the
output mesh is the table's, else **the model whose name hashes to the input** (`FUN_00539150(0x76c310, …)`), else no
event. TransformNumberColor: NumberColor(v, r, g, b) keys; TransformNumberVector3: NumberVector3(v, x, y, z) keys;
EventInput → EventOutput.

## 3. Per element (1playerhud; Viewport units; children add to their parent) [V data, M frame]
| element | node | where / size | art | fed by |
|---|---|---|---|---|
| **Health** | Group `player1healthbar` (0.064863, 0.827526), α 0.6, FadeIn 1 / Out 0.2 | fill BarBitmap 0.201996 × 0.014658 Left/Top, uv (0, 0.5)–(1, 1), z 2, rgb (0,150,213), pulse to black at `healthRegenPulseRate`; bonus fill z 1 rgb (0,150,240); empty backing 0.211292 × 0.029295 at (−0.004992, −0.007543), z 3, black α 0.6 | `hud_jetpack_and_energybar_fill` | `player1.healthFraction`, `.bonusHealthFraction`, `.healthColor`, `.health`/`.healthDisable` |
| **Stamina** | Group `player1energybar` (0.064532, 0.854368) α 0.6 | fill 0.201996 × 0.007658, z 253, α 0.75, rgb (209,168,41), FlashyScale 1; empty 0.212315 × 0.01669 at (−0.005518, −0.003194) z 255 black | same sheet | `.energyFraction`, `.energyColor`, `.energyRegenPulseRate` |
| **Ammo / weapons** | Group `player1weaponinformation` (0.068639, 0.427927) z 255 → `…_lineupwithhealth` (−0.070914, 0) | weapon1 total ammo Text `gamefont_small` black at (0.075284, 0.340939) z 6; weapon2 (grenades) at (0.076532, 0.286563); names `gamefont_tiny` α 0.75 FadeIn 0.25 / Hold 2.5 / Out 1 on `.change`; backdrop meshes at (0.2078, 0.356) / (0.2076, 0.370) Scale 0.2 α 0.98; weapon meshes in groups at (0.4037, 0.0052) / (0.4037, 0.0474), per-weapon MeshInfo (e.g. `rep_weap_inf_rifle` (−0.190, 0.360), rot (1.5, 95.7, 4.5), Scale 0.8); ∞ icon `hud_infinite_ammo_icon` | `hud_weapon{,2}_backdrop`, weapon models, `hudtransforms` table | `.weapon%d.totalAmmoBullets`, `.change`, `.name`, `.mesh`, `.ammoInfinite`, `.disable` |
| **Reticle** (RETC) | Group `player1reticule` (0.5, 0.410624) α 0.65, moved by `.weapon1.reticule.position` | Model3D `hud_main_reticule` Scale 0.26 α 0.52 colour ← `.weapon1.target.teamColor` (ColorChangeRate 0.001); bullseye, black outline α 0.5; hit / critical-hit flashes rgb (1,86,213) Hold 0.33 / 0.6; clip ring BarSegmented 30 × `hud_clipbar_segment` (Shadow) over −135.3°…144.8° in 0.0878 × 0.1114, value `.weapon1.totalClipFraction`, Hold 6.03; heat ring 30 × `white` −150°…150° rgb (239,255,41); refire bits | INGAME meshes + `hud_clipbar_segment`, `white` | see node |
| **Reinforcements** | Group `player1teams` (0.5, 0.099208) α 0.8 FadeIn 3 / Out 2 | friendly count Text `gamefont_medium` Center/Top rgb (1,86,213) at (−0.174945−0.051935, 0.027542−0.019856); enemy rgb (150,30,30) at (0.173132+0.052427, 0.030501−0.020526); black drop-shadow twins (+0.003, +0.002); team icons 0.07 × 0.09 from `.team%d.texture` | font only (icons by event) | `.team1.*` = the local player's team, `.team2.*` = the other [V] `FUN_003b59a0` D:411241–411252 (slot 0 = `PTR_DAT_005fee78[player+0x134]`, slot 1 = the other team); count = team +0x24 (0x7fffffff when +0x28 is), fraction = count / +0x28, pulse period = 1 / team +0x2c (FLT_MAX at 0) (D:411300–411326); sent on change, and only while the slot's ShowTeamPoints byte is 0 (default 0, `FUN_003b0aa8`; set by `ShowTeamPoints` via `FUN_003b75b0`) |
| **CP capture** | NOT in the layout: a code object draws the `hud_flag_timer` symbol at the right edge at mid height in every HUD, both games (`re-capture-display.md`, 2026-10-13 — this row said "1-player: NO bar", which was wrong: the layout has no element for it, the PS2 frame has the icon). The layout's own ring — 4-player only, Group `playerNcpstatus` with BarSegmented `playerNcpdisputebar` / `playerNcpcapturebar` (10 segments, 360°) — is dead data on the PS2: no code sends its events (`re-capture-display.md` 1–2) | 64 × 64 units, frame x 421–486, y 194–258 | `hud_flag_timer` | the object's own scan of the posts (not an event) |
| **Minimap** | Map `player1map` (0.856385, 0.186248) Scale 0.9 z 127 α 0.85; `PositionLarge` / `PositionSpawn` for the big and spawn views | backdrop = `<world>_map` masked by `map_mask`, uv (−0.311, −0.207)–(0.776, 0.880), α 0.5; FOV wedge `hud_map_fov` ×1.15; north pointer `hud_map_northpointer` at (0, −0.11) ×2; posts/targets/markers with Shadow | per-map `*_map`, INGAME sheets/meshes | `.map.*`, `.index` |
| **World markers** | Target `player1target` | objective / flag / attacker / locked-on icons on screen, behind, off screen (MinScale/MaxScale with distance) | `hud_objective_icon*`, `hud_target_*` | `.spawn`/`.die`, targets |

- [M] The frame agrees with §2.4's zero-parameter mapping (health fill, team counts, "147", reticle — the numbers are
  there); black "250" and "4" sit on pale backdrop pills with the rifle and thermal-detonator models
  (`crop_weap_hot1.png`), the counts are blue (friendly) left and red (enemy) right.
- [V][O] Weapon ammo numbers (`FUN_003b3398` D:409753 ff.): the weapon's ammo state (+0x90) keeps clips: +0x8 max, +0xc
  reserve (FLT_MAX = infinite), +0x10 the current clip's fraction; RoundsPerClip-like count = class +0x98.
  totalAmmoBullets = (int)((reserve + clip) × rounds) — clip and reserve in ONE number; totalClipBullets = (int)(clip ×
  rounds); fractions likewise; infinite (count ≥ 2³¹ or class +0x9c = 0) sends −1 and `ammoInfinite`. On a weapon change it
  sends `.change` with the weapon's vfunc +0x98 value and the localised name (class +0x60, else the class name at +0x40).
- The port sends the localized names from the match's text tables
  for both rows. Class identity can refresh the original name fade even
  when two weapons share the same HUD mesh. Icon-sheet fallbacks still
  send the same change event, so the labels use the authored placement
  and 0.25 s fade-in, 2.5 s hold and 1 s fade-out.
- [V][O] **Which mesh a weapon shows** (second Opus verifier, 2026-09-30, verdict PARTLY on my first wording — this is its
  corrected rule): `.change` carries the weapon's vtable +0x98 value (D:409827, sent at D:409858). For 19 of the 20 BF2
  weapon vtables that is `FUN_00326e28` (D:318603): class+0x68 if non-zero, else class+0x64. class+0x68 = fnv(`HUDTag`),
  set by the weapon-class setter `FUN_00328fb0` (D:320624, hash 0x9ff840d6) and inherited through ClassParent (copy ctor
  D:319643); class+0x64 = fnv of the class's OWN `GeometryName` (D:320049–320070, not inherited — the copy ctor puts the
  new class's name hash there, D:319638–319642), else the class-name hash (root ctor D:319497; `WeaponName` 0xfbf47dba
  would override, no BF2 class uses it). WeaponDetonator (vtable 0x628880) uses `FUN_00333648`: while its charges are out
  it sends class+0x4c0 = fnv("hud_detpack_plunger") (root detonator ctor `FUN_003340c8`, D:327085). [M] RAM (HOT1): 55/55
  live classes fit (`imp_weap_inf_rocket_launcher` → fnv("imp_weap_inf_launcher"); vehicle cannons → "hud_vehicle_lasers";
  `all_weap_inf_invisibility` → "hud_stealth"). 53 of 291 baked BF2 weapon classes have neither HUDTag nor their own
  GeometryName, so they send their class name — which is why `hudtransforms` mixes class names
  (`all_weap_inf_thermaldetonator`) and geometry names. The port's class RONs carry HUDTag as prop 2683846870.

## 4. For the build (port decisions — reversible, not PS2 facts)
- Bake `hud_` → RON as a generic tree (type, name, args, children) with the names of §1.1; fonts → glyph sheets +
  metrics; the five COMMON sheets, `white`, and the side `hud_*` meshes.
- Layout on a 4:3 box of the window height H (width 4/3 H): the PS2's own mapping (§2.4) is x = (−0.0059 + 1.0139·fx)·(4/3)H,
  y = (−0.0242 + 1.0484·fy)·H; element-local sizes (BitmapRect × 512/448 px, glyph texels) scale by H/378.7 across and
  H/427.3 down; a Model3D unit is 0.2129 H across and 0.1887 H down (orthographic is close enough at depth 6.8 units vs a
  mesh 1 unit wide [D]). Then **anchor** each top-level element to the left edge, the centre or the right edge by its fx
  (< 1/3, middle, > 2/3), keeping its 4:3 offset from that anchor (Hor+, never stretched, COMPARISON_RULES). Whether to
  keep the PS2's 1.128 horizontal stretch of HUD meshes and texel glyphs is a look call for the build (default: keep —
  it is the PS2 frame; one constant each).
- Text: keep the drop-shadow twins the data already carries; TextStyle Shadow draws its own (+1, +1) black copy only for
  bitmaps as read (§2.2) — text Shadow is [H] the same.
- BF1 maps: the same BF2 tree; the minimap uses the BF1 map's own `*_map`; a BF1 weapon with no BF2 mesh and no
  `hudtransforms` entry shows its BF1 `IconTexture` sheet (e.g. `IMP_blasterrifle_icon`, BF1 COMMON) as a Bitmap in the
  weapon slot (PLAN §Presentation).
- Feed: health/max, stamina, clip+spare as one number, clip fraction, weapon class + name, reinforcements friend/enemy
  and the bleed period, the reticle pick colour (AIMF's `reticle_pick`), the minimap's posts.

## 5. Checks run (paste) and what stays [H]
Full outputs: `engine/artifacts/lead_tmp/re/bhud/checks.txt` (re_check) and `checks2.txt` (disc searches).
1. `re_check.py fn b2` on 61 cited lines — every one sits in the function this doc names (e.g. *(decompiler detail omitted)*, *(decompiler detail omitted)*,
   `B2:412862 is in FUN_003b8040 … & 0xfffffffffffffffc | 2`, *(decompiler detail omitted)*).
2. Absence, by search: `re_check.py absent` — 0x46544626, 0x1eccdefd, 0x3bd46d91, 0x4b4bf31a, 0x625fa794, 0xca0b9ccb,
   `FUN_003b8040`: 0 hits in B1 (the BF2 HUD element model has no BF1 twin); a walk of all 119 BF1 LVLs (top level and
   `lvl_` children): 0 `hud_` chunks; a raw 4-byte search of the five `hud_` chunks: 0xca0b9ccb 0, 0x0e0d9594 0,
   0xf3c47270 0; `commandPost` in the dump: only chunk @0x321760 (`4playerhud`, 15 lines);
   `hud_target_hint_offscreen` on the whole BF2 disc: 12 hits, all inside INGAME.LVL's `hud_` span 0x2ddbd8–0x350d48 (the last chunk ends the file).
3. The twin: BF1's HUD is not this model (item 2). Where a claim is BF2-only it says so; BF1 maps get the BF2 HUD by PLAN.
4. Words: 0x5ff850 → "Pixels" "Screen" "Viewport" "Frame"; 0x5ff848 → "Alpha" "Additive"; 0x5ff868 {Normal, Shadow};
   0x5ff870 {Left, Center, Right}; 0x5ff880 {Top, Center, Bottom}; 0x5ff9b0 {None, Word, No Reformat, Word Reformat};
   0x5ff9c8 {Left, Right, Center}; 0x5ff9d8 {Top, Bottom, Center, Baseline}; 0x5ff9e8 {Normal, Shadow, Selected};
   0x60c7b0..c0 f32 1.02, 1.0, 1.24, 1.23, 1.3; 0x650970 slot +0x40 (0x6509b0) = 0x0059fe60; 0x62fc20 "%s_map";
   0x62fc28 "map_mask". RAM (bss, not in the ELF): 0x7d5f70/74 = 512/448, 0x7d6208/0c/10 = 1.7321/1.1547/0.8660,
   0x7c56e8 = 0.7666, 0x76c220 = 00 00 00 ff, 0x825da0 = "hoth".
5. Writers: DAT_0060c7b0..c0 — 0 text-visible writers (ELF .data values stand); DAT_0060c774 — only `FUN_0059fd80`
   (D:735050, from the root flag at D:737961–737962); DAT_007c56e8 — only `FUN_00580b08` (0x3f443fe6); DAT_0076c220 —
   only `FUN_0057ca38` (0); DAT_007d6208/0c/10 — only `FUN_005a45d0`; DAT_005ff860 — `FUN_0039ec80` (screen width, and
   × 0.5 at D:396720 — a split-screen halving [H]). Pointer writes are invisible to these searches; the RAM values agree.

Still [H]: (a) resolved (§3, second verifier); (b) Viewport(1) = root
entry 0; (c) text Shadow = the bitmap Shadow's (+1, +1) black copy; (d) the team icons show only when DAT_005fed90 ∈
{2, 3} (D:411264 — a mode test, not traced; the conquest frame shows none); (e) what the PS2 draws for the missing
`hud_target_hint_offscreen`; (f) the names of the 11 unnamed hashes; (g) BarBitmap flash timing beyond the two slots.
PROVISIONAL for code: every [D]/[M] number above (the mapping constants are ELF/RAM values, their combination is derived).

## 6. Verifier (one Opus agent, foreground, claims + citations only — 2026-09-30)
21 claims (C1 layout chunks + no BF1 `hud_`; C2 registry + BF1 absence; C3 common properties; C4 Position modes; C5
Rotation; C6 z-order; C7 fader; C8 colour/alpha + defaults; C9 bitmap tables; C10 Shadow pass; C11 BarBitmap fill; C12
Text tables; C13 Model3D + the VMULx.xyz masks from the ELF bytes; C14 the HUD camera terms; C15 root selection; C16
minimap name + RAM; C17 NameMesh fallback; C18 team slots; C19 ammo formula; C20 fonts; C21 the two string absences).
**20 SUPPORTED, C15 PARTLY** — bit 35 is also cleared by the ctor and copied from a template, so "written only by" was
wrong; the conclusion (every HUD element joins the first root array) stands: 0xca0b9ccb is in none of 165 BF2 LVLs and
only one compare in the ELF. Folded in: state 4 gives α 0 once t ≥ FadeOut; the colour/alpha update runs only while the
render element is visible; the bar flash also needs +0x370 bit 35; the team events are change-gated and muted by
`ShowTeamPoints`; the case-sensitive "commandPost" note; the `hud_` span ends at 0x350d48; the group draw (0x5a2168, not
in the corpus) walks its children head-first too, so z-order holds inside groups. Verifier scratch: `/tmp/hudv/`.

Second verifier (weapon-mesh key W + NameMesh fallback T): W PARTLY — the plumbing, `FUN_00326e28` and the HUDTag field
SUPPORTED; "every weapon vtable" wrong (20 vtables, the detonator's differs) and "+0x64 = GeometryName" only for the class's
own GeometryName, else its name; T SUPPORTED (`FUN_003bafa8` falls back to the model table also when a matched entry's
mesh is not loaded). Folded into §3.

## 7. Numbers for the build's acceptance
`hud_` records (DATA nodes) / top-level elements: hudtransforms 125 / 1, 1playerhud 2,343 / 42, 2playerhud 4,722 / 4,
3playerhud 9 / 1, 4playerhud 5,271 / 6. Fonts: 4 × 226 glyphs, pages 1 / 3 / 3 / 2. PS2 frame boxes (HOT1 postA v1,
640×480): health fill x 39–169, rows 406–413; stamina fill rows 420–422; friendly count centre x 172.5, rows 46–59; enemy
centre x 466.5; reticle outline 76 × 70.

## 8. Shared source layout (November 2026, issue #114)

The single-player HUD uses the PS2 geometry in every source profile and in both
games. This is an openbf presentation decision, not a claim that the original
PC and Xbox games used the PS2 layout. `bake/src/hud_layout.rs` applies only to
stock `1playerhud`, with edition evidence from native vertex buffers and stock
weapon/map anchors. PS2 input is a strict no-op. Other layouts and modified
stock anchors remain untouched. A rebake/repack is required for existing
saved layouts and installed packs; the runtime does not migrate older bakes.

The correction table retains only numeric geometry and alignment names for
properties consumed by `ui::hud_tree::Hud::element` and `ui::hud_draw`:
placement with its coordinate mode, rotation, scale, viewport-relative
rectangles, text alignment and segmented arc geometry. These have the PS2
property semantics traced in §2.1–2.4. MeshInfo corrections are keyed by their
model parent as well as mesh name: the primary and secondary rows can name the
same vehicle mesh with different transforms. Within one model the first
binding wins, as in the renderer; unused later duplicates remain untouched.
No asset or localized text is
retained in the table. Mesh, bitmap and texture-coordinate bindings, root
events, fades, colors and alpha remain native. The established PC ammo/name/
team-count font roles and layout scales remain, using the player's PC atlas.
The same compatibility scales apply to the matching statistics, message and
distance roles. The message box fits seven font lines with PC tiny at 1.14
vertical scale (13 × 1.14 = 14.82 pixels) and PS2 tiny at 15 pixels; both give
a 16-pixel pitch in its 112-pixel rectangle. Queue capacity and fades stay native.
Selected font files and metrics stay native. PC cap bearings are read from the
selected baked font's `0` glyph; the PS2 numeric reference is tiny line/top
15/4, small and medium 18/3, large 22/3. The established role scales are kept;
large uses the measured numeric advance 10→13 and cap height 12→15. Element
placement compensates the scaled bearing and vertical alignment, including a
Text's own scale and a multiline format's line pitch. This aligns cap tops
without replacing a font face. Glyph widths, descenders and mesh silhouettes
can still differ. Native width/height variation is not pixel-identical art.

The PC secondary backdrop's 22 native vertices have a different Y origin:
its bounds are −0.673712..0.505169 model units, against PS2's
1.131230..2.310093. The shared layout position alone leaves it 32.7 display
pixels too low at 640×480. A −29.11197 frame-pixel placement correction at the
existing 0.200001 scale compensates the measured 1.8049331-unit pivot offset.
Its native binding and vertices remain untouched. Native cap positions and
pill bounds are tested through `HudView`, since equal raw configuration
positions alone do not establish rendered spacing.

The PC weapon children lack the PS2 `player1weaponinformation_lineupwithhealth`
group. The bake inserts that relative group at X −0.070914 and sets the root
at (0.068639, 0.427927), retaining its original enable/disable events. This
avoids applying the root correction without its compensating relative offset.
The Xbox default map Position (0.303257, 0.477156, 0) is its spawn position;
its separate Small position is not consumed by the current interpreter. The
shared Small anchor is (0.856385, 0.186248, 0.004111), scale 0.9, preserving the
renderer’s existing right-edge Hor+ selection. Xbox statistics Y changes from
1.011855 to 1.155109. Health, stamina, jet, ammo/pills/models, teams, target,
countdown and message geometry are corrected by the same path. The reticle’s
runtime aim-ray position event remains authoritative over its authored origin.
The existing shared placement of victory and defeat labels also remains.

`Target`, `ObjectiveList`, `BorderedBox`, the unnamed hero pulse type and
`VehiclesSeating` are outside the current renderer’s scope. Their uninterpreted
geometry and Map child view/style records are excluded from normalization;
code-drawn capture, hit-direction, squad, scope, spawn and round-end overlays
continue to use their own shared geometry. Missing native MeshInfo bindings and
shadow-twin elements are not synthesized. The vehicle health bar can interpret
its native events, but `HudModel` does not yet publish vehicle health; this
layout change does not add gameplay publishers. Item/backdrop depth ordering
is a separate change.

The four-source CPU regression loads real baked trees, checks the interpreted
geometry and matching MeshInfo transforms against the PS2 control (with the
intentional PC cap/pill Y compensation), and measures
native-art bounds at 640×480, 1280×720 and 3840×2252. Infantry, vehicle weapon
channels and disabled states cover both games. `ui_sharp` uses the same models
for headless GPU captures. The original PS2-frame tolerances in
`game/tests/suite/hud_layout.rs` remain unchanged. These are source/renderer
checks; fresh original-game and packaged visual acceptance remain separate.
