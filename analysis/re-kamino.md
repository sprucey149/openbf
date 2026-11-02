# OPUS RE (2026-09-30) - [O] UNAUDITED — Kamino bring-up (BF2 and BF1 KAM1): the sky, the sea, the floors, the far pieces

Card KAMR (lane 3). The maintainer's play session on BF2 Kamino (`lead_tmp/playtest_0930/kam1_play_5..11.png`): "invisible
floors/platforms, far image looking wrong, the sea nearby is wavy but then there's a hard cutoff and it looks flat and a
different colour, things aren't rendering or appearing where they are supposed to". Parity at main 0994d9a: bf1_kam1
26.62, bf2_kam1 22.49 (`scripts/parity.py run --maps bf2_kam1,bf1_kam1`, reproduced). Every port number below is at
0994d9a unless it says otherwise; BPNT and TBIND landed during the pass — at 5c73167: bf1_kam1 26.62, bf2_kam1 20.80.

Scratch: `engine/artifacts/lead_tmp/kamr/` — `re/` (fxkeys.py, ramsky.py, claims.md, VERDICTS.md, checks.txt),
`ps2exp/` (kamr_ps2.py, `out/` frames + states + CSVs), `tools/` (floor_census.rs, kamr_pose.rs, kamr_walk.rs,
dome_prototype.patch — scratch examples, built in the detached worktree `openbf-wt/kamr-scratch`), `census/`, `sbs/`,
`views/`, `parity_*`. Tags: [V] read (lines cited), [V-RAM] read out of PS2 RAM, [V-PS2] measured on PCSX2 frames or
live RAM, [D] derived, [H] hypothesis. B2 = `analysis/SWBFRNT2.ELF-decomp.c`, B1 = `analysis/SWBFRONT.ELF-decomp.c`.

## 0. Answer

| what the maintainer saw | cause | PS2 | port | card |
|---|---|---|---|---|
| the sky far too bright (not in his list; the largest parity term) | `Effect("Lightning")` darkens dome shell 0: colour 255·(1 − `SkyDomeDarkenFactor` 0.4) = **153** between flashes (§1) [V][V-RAM] | sky ≈ 0.55–0.6 × the texture | every shell at white | **KSKY** |
| "wavy sea, hard cutoff, then flat and a different colour" | past the far pass the port draws its OWN still plane; the PS2 draws no water past whole 160 m tiles reaching 0.8·FarSceneRange (640 m) and the frame is black there (§2.1–2.2) [V][V-PS2] | sea fades into black at ~640–680 m | wavy to 640 m, then a flat unfogged plane to the horizon | **KSEA** |
| (same view) the sea 2.5× too bright, white foam | BF2's ocean is LIT (the packet carries FFT normals; colour ≈ texel × sun) and its far part is fogged with the world fog colour; the port draws it unlit at unity and unfogged (§2.3) [V][V-PS2][D] | (8,19,19) at 314–393 m | (26–49, 38–57, 34–55) | **KSEAL** |
| "upside-down bell tower, dark poles hanging" | AUTHORED: `kam_bldg_tower1` is a cluster of spires modelled hanging; 5 of BF2's 7 instances are flipped to stand on the mushroom domes, 2 hang beside platforms. PS2 and port agree (§3) [V][V-PS2] | same | same | none |
| "invisible floors/platforms" | the port's ground query is ONE ray: it falls through slits between collision floor pieces (the cloning rooms) onto the hidden soldier-only disc 7.9 m below; the PS2 soldier stands on the slit (§4.3) [V-PS2]. The rails ARE walkable on the PS2 (§4.2), the hidden discs / cores / wall boxes are the disc's own prims (§4.1) | stands | falls when standing or stopping on a slit | **GRND** |
| BF1: walking on air over the sea | BF1 KAM1's y = 0 heightfield (152,371 m² of reachable floor with nothing drawn) — `TerrainEnable(0)` (re-terrain-enable.md) | no terrain floor | invisible floor | TENS (exists) |
| the grey ellipse in the sky (`kam1_play_7`) | NOT FOUND at any stored pose [H]; ruled out: map models with a missing texture (none), the three sky vehicle pads (campaign layer only), the sky shells | — | — | note on KAMR |

No misplaced object was found: at the stored poses (70: 24 + 7 BF2, 28 + 11 BF1; 52 viewed at full size — 30 of
BF2's 31, 22 of BF1's 39 — the rest as thumbnails; sheets `lead_tmp/kamr/sheets/`) the geometry of both games registers
with the PS2's (§5). What differs is look: sky, sea, bloom, local lights, rain, haze.

## 1. The sky dome is darkened by the Lightning effect [O]

**Disc** [V bytes; `re/fxkeys.py <LVL>`]: BF2 `KAM1.LVL` `fx__` @0x7734c and BF1 `KAM1.LVL` @0x57960 author the same
`Effect("Lightning")`: `Enable(1)`, `Color(255,255,255)`, `SunlightFadeFactor(0.1)` (0x15f3deb6),
**`SkyDomeDarkenFactor(0.4)`** (0x034aebfd), `BrightnessMin(1)`, `FadeTime(0.2)`, `TimeBetweenFlashesMinMax(3,5)`,
`TimeBetweenSubFlashesMinMax(0.01,0.5)`, 0x67f9208d(2,5), `HorizonAngleMinMax(30,60)`, two sounds; plus
`LightningBolt("skybolt")`, `Precipitation` (Streaks, density 80, velocity 8, length 1.7, colour 216/220/228, ground
effect `com_sfx_rainsplash`), `Wind`, `ColorControl`. BF1 `YAV2.LVL` authors the same Lightning block (@0x7b460) —
the verifier's find, reproduced. No other map LVL does.

**BF2 code** [V]:
- Parser `FUN_0045aa30` (519790): 0x34aebfd → L+0x38 (519857–519859), 0x15f3deb6 → L+0x34 (519861–519863),
  FadeTime → +0x40, flash interval → +0x44/+0x48, BrightnessMin → +0x3c, Enable → `FUN_00451678` (registers the tick).
- Tick `FUN_0045ae80` (519989; timer t = L+0x64, next flash L+0x68):
  - first tick: saves the sun colour (`DAT_0060c6e0`+0x1c), the second light, the two ambients, the sun direction and
    the fog colour (`FUN_005804d0` → L+0xe0) (520056–520077);
  - while t < FadeTime: lights blended toward the flash colour, and
    **dome colour = white·(1 − f) + black·f, f = L+0x38 · (1 − b)**, b = brightness·(1 − t/FadeTime) + jitter
    (520289–520300); the FOG colour is set to the same lerp (`FUN_005804b0`, 520301–520313);
  - on the tick t passes FadeTime: lights and fog colour restored, **dome colour = lerp with f = L+0x38** (520152–520170).
- The constants: `DAT_0076c200..203` = (ff,ff,ff,ff), `DAT_0076c220..223` = (0,0,0,ff) (`FUN_0057ca38` 712404–712432;
  one store per byte, ELF scan agrees).
- Consumer: the sky tick `FUN_003cb918` draws **shell 0** with `*(sky+0x2855)` (425999–426002) and every other shell
  with the constant white (426008–426009). `0x2855` has exactly four uses: the ctor (416068), this read, the two stores.
- So between flashes shell 0 draws at **153/255 = 0.6**; `kam1_city` (shell 1) stays white.

**BF1 twin** [V]: parser `FUN_00377c40` (+0x2c 366740, +0x28 366748); tick `FUN_00378120` writes
`*(DAT_00564670 + 0x100)` (367027–367042, 367285–367294) from `DAT_0069c420`/`0069c440` (`FUN_00492598` 555048–555075);
sky tick `FUN_002f2308` draws shell 0 with sky+0x100 (280282–280284), the rest with 0x69c420 (280278–280279).

**PS2 RAM** [V-RAM] (`re/ramsky.py`): dome colour (153,153,153,255) in 7 of 7 BF2 KAM1 in-level states, 9 of 10 BF1
KAM1 states and 4 of 4 BF1 YAV2 states; BF1 KAS2 (no Lightning) 255. `bf1_kam1_walk_cp1_cp10_b000` was saved at a
flash's peak (dome 255, fog colour 255); three more mid-flash: `bf2_kam1_cw_sideselect` 224, `bf1_kam1_cw_cis_spawnmap`
245, `bf1_kam1_cw_sideselect` 193 — the fog colour equals the dome colour in each, as the code says.

**On screen** [V-PS2]: sky boxes at the BF2 walk poses read PS2 (54,69,72) / port (98,124,128), (28,40,45) / (50,72,77),
(50,65,68) / (93,119,124), (38,49,52) / (62,85,90) — ratio 0.52–0.61. Top-left sky band, median PS2/port over the walk
views (`skyratio.py`): ~1.0 on 29 maps, **bf1_kam1 0.47, bf2_kam1 0.76**. (bf2_nab2 also reads 0.54 — not followed.)

**The port** [V]: `viewer::scene::upload_sky` gives every shell `color: sky.ambient` (white on all 32 maps; the PS2 has
no reader of `DomeInfo.Ambient` at all — its hash 0x1c9640eb is in neither decomp); nothing reads the Lightning block.

**Prototype** (`tools/dome_prototype.patch`, shell 0 × 0.6, on 0994d9a): parity **bf1_kam1 26.62 → 21.80 (−4.82),
bf2_kam1 22.49 → 19.20 (−3.29)**;
sky band ratio bf2 0.76 → 0.94, bf1 0.47 → 0.64. BF1 keeps a gap (0.72 with the BF1 filter off too) — open, §8.

**Built (KSKY, 2026-10-01)** — what the build added to this section:
- The unnamed key 0x67f9208d is **`NumSubFlashesMinMax`** [V: fnv1a-lower of the name; stored as two ints at L+0x54/+0x58,
  519914–519918].
- The ctor's defaults ([V] the stores; [D] that `FUN_0045a7d0` is this effect's ctor — its offsets are the parser's;
  no verifier pass, the port consumes none of them) (B2 `FUN_0045a7d0`, 519747–519758): colour (239,248,255)/255 at L+0x24, sun fade 0.1, **dome
  factor 0.4**, brightness min 0.9, fade 0.2 s, flashes 3..8 s, sub-flashes 0.08..0.16 s, 2..5 of them, horizon 30..60°
  (stored in radians), not enabled. The three maps author every key, so no default is in play. BF1's ctor not opened [H].
- The factor is a GS (gamma-space) multiply, so the port's linear pipeline tints shell 0 by sRGB⁻¹(153/255) = 0.319, not
  0.6 (the rule `ReflectionColor` already follows). The prototype's linear 0.6 was a display factor of 0.79 — which is
  the whole of §8's "BF1 stays 0.64–0.72" item: with the linearised factor the top-left sky band reads PS2/port
  **0.96 on bf1_kam1 and 0.96 on bf2_kam1** (main bc6a047: 0.47 / 0.76), and parity moves bf1_kam1 27.00 → 19.32,
  bf2_kam1 20.88 → 16.29, bf1_yav2 7.30 → 7.22, no other map (`lead_tmp/ksky/`, report `20261001_2130_card_KSKY.md`).

## 2. The sea [O]

### 2.1 PS2 frames (`ps2exp/kamr_ps2.py sea2`; BF2 KAM1, 0-AI state, SW renderer)
The soldier held in open air past cp5's rim: eye (21.86, 72.24, −118.94), yaw 0.02, pitch −8.28, fov 52.06, looking out
to sea. The Lightning effect frozen (L+0x68 = 1e9; L found by its float pattern, 0x173a750) so no flash moves a frame.
Mean RGB over x 0..110, 4 rows, by flat-sea distance at y = −8 (the real surface is a few metres higher):

| distance m | 314 | 363 | 393 | 428 | 470 | 521 | 585 | 665 | 770 | 913+ |
|---|---|---|---|---|---|---|---|---|---|---|
| control (3 frames, ±2) | 8/19/19 | 9/19/19 | 9/19/19 | 6/15/15 | 6/14/14 | 5/13/13 | 4/11/11 | 3/5/5 | 2/3/3 | 6/6/6 |
| fog colour (0,255,0) | 6/16/15 | 6/17/16 | 6/17/16 | 4/15/12 | 6/21/13 | 5/32/13 | 6/43/12 | 4/52/8 | 5/10/6 | 6/6/6 |
| fog colour (255,0,0) | 7/17/17 | 7/17/17 | 8/18/18 | 11/18/18 | 14/17/17 | 23/17/17 | 36/16/16 | 46/10/11 | 11/5/5 | 6/6/7 |
| `FarSceneRange` 800 → 400 (first run, pitch −10.19; nearest row) | 7/17/17 | 9/18/18 | 7/12/11 (384 m) | 5/5/5 (409 m) | 3/3/3 | 5/5/5 | 6/6/7 | 6/6/6 | — | — |

- **The sea ends; beyond it the frame is black** — also at a flash's peak, when dome and fog are white
  (`out/sea_ctl2.png`): the band between the sea's edge and the horizon is the frame's clear colour, not fog, not sky.
- **The far part takes the world fog colour**: green / red / white pokes add 47 / 43 / 39–46 levels at the edge,
  ~31 at 585 m, ~19 at 521 m, ~7 at 470 m, 0 at 428 m — 17–18 % at the edge, starting near FogRange's 400 m. With the
  authored black the sea simply darkens into the black behind it: no visible edge. During a flash the edge band turns
  light grey (33/38/37).
- **The edge follows `FarSceneRange`**: 800 → the first black row is at 736 flat-m (sea at 661); 400 → black at 409
  (sea at 384). The eye is at z = −119 looking down −z, so the 160 m tile rows end at 681 and 361 m: the pass draws
  WHOLE tiles that reach into the range, and the sea ends on a tile boundary between 0.8·Far and 0.8·Far + 160 m [D].
- **The sea is dark and blue-shifted**: (8–9, 19, 19) where the port's unlit base texture reads (21,34,29) at the same
  pose — ratios (0.38–0.48, 0.56–0.59, 0.66–0.69), i.e. the sun colour (65,85,105)/128 × ~0.8. In frames taken with the
  lightning live the sea goes to (2,9,7) at a flash's start and to (22–25, 38–42, 34–38) — the bare texture — later in
  one (`out/sea_fogred.png` of the first run): it follows the lights.

### 2.2 The far pass [V]
`FUN_00575670` (708275), far branch *(decompiler detail omitted)* (708400; called from the sky draw through `FUN_00560540(0x20000)`,
426026): range `[… tiles
counted by `FUN_00561b58` (708405; `DAT_0066164c` reset 696830, +1 per tile 696895); with ≥ 1 tile it binds three
programs and a record {0x144, TEST 0x50010} whose colour is the fog colour `FUN_005804d0` × the HDR scale
(708435–708451), and a texture named "fogspread" (0x64fd60; 708410–708412). BF1's twin is re-water.md §1 (551039, 551047).
The fog colour `DAT_007bb7f0` has one writer, `FUN_005804b0` (715477): the `sky_` FogColor and the Lightning tick.

### 2.3 The ocean is lit in BF2 [V packer, V-PS2 colour, D rule]
`FUN_00574a70` (707985; callers 708144/708145) writes per vertex the packed position, **three normal bytes
`(char)(n·127)`** from the FFT normals (708056–708058), the UV and the colour `(white + 1) >> 1` with alpha
`(foam byte + 1) >> 1` (708067), and submits them through the generic mesh call with a normal stream
(`FUN_00494af8(…, positions, normals, uvs, 0, colours)`, 708105). §2.1's colours say what the normals are for.
The exact rule (which lights, ambient or not) is NOT read: build it PROVISIONAL and fit §2.1's table.
BF1's ocean stays the unity draw re-water-colour §1.9 found (PS2 (17,30,33) vs port (20,33,29), KOC).

### 2.4 The port [V]
- `game/src/water_anim.rs` header: past `0.8·FarSceneRange` the port "keeps drawing the still plane it always drew …
  THAT plane is the port's, not the disc's". `OPENBF_FAR_CULL=1` removes it and the frame then matches the PS2's
  structure (`views/sea_compare.png`: sea, edge, black, the far city's stilts over black).
- `viewer/look.rs::water_look`: a BF2 ocean gets `fog_begin: None` (never fogged); BF1 gets `NearSceneRange.c`.
- The ocean is drawn at unity, foam white: (26–49, 38–57, 34–55) at the pose, against the PS2's (8,19,19).
- `examples/pose_render.rs` never calls `animate_water` (headless does on its bench and `--cam` paths only): every
  parity render of KAM1 shows the flat still plane, so parity cannot see the sea the game draws.

### 2.5 What KSEA built (2026-09-30) — the four bullets above describe main before it
- A stepped ocean draws whole 160 m cells whose nearest point is within 0.8·FarSceneRange of the eye and nothing past
  them (`water_anim::cells_in_range`); an unstepped one keeps its flat plane, cut per fragment at the same range.
  **Ours:** the distance is radial where the PS2 tests its frustum (the tile test is unread) — the same row down the
  view axis, and the edge does not move as the camera turns.
- A BF2 ocean takes the world fog past NearSceneRange.c (`water_look`). Measured at the §2.1 pose, fog colour green,
  x 0..110, G gained over the control at 682 / 638 / 598 / 563 / 532 / 504 m: port 25 / 41 / 35 / 29 / 17 / 10, the
  PS2's second grab (`s2_foggreen1`) 30 / 43 / 39 / 31 / 23 / 18. (§2.1's first-grab frames still show the old fog —
  `s2_foggreen0` gains 1–5.)
- `pose_render`, the map sheets and the walkabout step the water to 0 s from each eye.
- At the §2.1 pose the bands behind the sea read 2.6–9.1 (PS2 2.2–9.2) and the sea's last band is the 682 m one (PS2
  637–682). BF1 matches its PS2 pair at the edge (`cp0_cp2_beta1_v3_water`, x 400..440: black to row 244 / 240, then G
  15, 26, 28, 33 / 21, 53, 26, 33). **BF2's edge still shows:** the port's sea is (25–27, 29–36, 27–33) there, the
  PS2's (3–4, 5–8, 5–8) — edge contrast 32.5 against 3–6. The PS2's sea darkens with distance by more than its fog
  (G 19 at 383 m, 8.4 at 598 m; the fog alone leaves ~16): KSEAL. RAM in `s2_ctl0.p2s`: `DAT_007c5754` = 0.5 and
  `DAT_0076c258..264` = (1,1,1,1), the two factors of the far tiles' colour word (708447–708467) — read, not traced [H].

### 2.6 What KSEAL built (2026-10-01) — the lit sea, a fit [V-RAM][V-PS2][H rule]
**Read on the way** (`re_check.py fn`: 708447, 708473, 708535, 708557 are in `FUN_00575670`; 697465 in `FUN_00562760`):
- Both branches of `FUN_00575670` send the same colour command — `DAT_0076c258..264` × `FUN_0057fd50()` (the HDR
  scale, `re-model-lighting.md` §1) × 128, packed twice (near 708538–708557, far 708461–708479) — so the sea's material
  colour is the white every lit segment gets, and the far branch adds only the `fogspread` layer. The three "programs"
  of §2.2 are texture records (`FUN_00497bd8`, `re-texture-wrap.md`), not lights. Which VU program lights the sea, and
  with which lights, is NOT read [H]. No verifier pass was run on these lines: nothing in the build rests on them.
- The HDR composite doubles the half-scale frame back (`re-model-lighting.md` §1), so the 0.5 is neutral for colour.
  The command's ALPHA is halved too and a blend weight is not doubled back: a lead for the faint foam below [H].

**The PS2's sea in RAM** (20 BF2 KAM1 states, `lead_tmp/kseal/ram_sea.py`): grid 17 × 17 (`DAT_006615e0/e4`), the
first tile's box (`*DAT_006615d0`) spans **8.8–19.6 m in y, median 15.2**; foam bytes (`*DAT_00661654`, 289) non-zero
on 76–107 points, mean byte 27–36. The port's field (`lead_tmp/kseal/kseal_stats.rs`, 20 times): 11.8–24.7 m, median
18.7; non-zero 76–105; mean byte 28–42. So the port's waves and foam are the PS2's (`re-water.md` §3a stands; the
port's range runs ~20 % high on these samples — not chased). At `s2_ctl0` the PS2's range is 10.2 m, the port's 17 m.

**What the frames say** (`s2_ctl0..2`, lightning frozen; `lead_tmp/kseal/accept.py`):
- Away from the city's lights (x 400..600, 215–383 m) the sea is (6.6, 16.7, 16.2) where main's unlit draw is
  (35, 45, 42); under the lights and the densest rain (x 0..110) it is (10.3, 19.6, 19.3).
- The foam is there and faint: no white crests anywhere in the three frames, with the same foam bytes in RAM.
- From ~385–400 m (`FogRange` 400) the sea darkens, by row medians, to 0.79 at 436–479 m, 0.73 at 504–532, 0.67 at
  563, 0.55 at 598, 0.42 at 638 of its 215–380 m level. The fog's colour pokes (§2.1) add only 0–18 % over the same
  rows, and at `FarSceneRange` 400 the sea at 363 m is not darker — so the loss follows the fog's range, not the far
  pass's, and is 3× what the poke gains account for. Cause unread [H].

**The rule built — PROVISIONAL, three constants in `viewer::look::water_look`:** a BF2 ocean's base and foam are
scaled in GS terms by `sun byte / 128 · max(n·l, 0) · 0.75` (the port's FFT normals, the manifest's sun, no ambient);
the foam layer blends at 0.5; past `FogRange.near` the colour falls linearly to 0.5 at the far pass's end
(0.8·FarSceneRange), in planar depth, before the fog. BF1's ocean is untouched (byte-identical frames).
Measured after (median of 3 water phases, x 0..110 band means at 314 / 363 / 393 / 428 / 470 m): port
(7.8,16.0,17.5) (8.7,16.8,18.7) (8.7,16.7,18.7) (7.3,14.3,15.9) (6.4,12.3,13.8) against the PS2's (9.3,19.5,19.2)
(9.1,18.6,18.4) (8.0,17.5,17.5) (5.8,14.1,13.8) (6.2,14.5,14.2) — worst channel 3.5; the last tile row
(3.7,5.3,6.0) against (5.4,5.8,6.1). **Residual:** the port's sea is bluer (B over G by 2–3 levels where the PS2's
G ≥ B), and x 400..600 reads (9.3,17.4,19.6) against (6.6,16.7,16.2).

## 3. The far pieces [V][V-PS2]
`kam_bldg_tower1` (GLB x 10..19.5, y 28..60, z −2..8.5: spires, wide end at the TOP, lit tips at the bottom) is placed
7 times on BF2 KAM1: `ant90/10/160/17/170` with rotation column 1 ≈ (0,−1,0) — flipped, standing on the two mushroom
domes (the dome surface at 107.1 / 74.3 / 91.1 inside their 103.3–134.7 / 68.5–100.5 / 88.3–119.7 spans) — and
`ant180/190` upright, hanging beside platforms (y 36–68, 28–60). BF1 has 4 flipped. At `walk/cp4_cp4_b270` the PS2
shows the same spires in the same place over the dome (`crops/bf2_kam1_walk_cp4_cp4_b270_380_0_540_150.png`); the PS2's
are darker (far scene), the port's lit strips brighter. Authored — nothing to fix. The same holds for `kam_prop_pillar`
/ `pillar2`, whose vertices sit 15–20 m off their origin with their collision prims at the same offset.

## 4. Floors [O]

### 4.1 The census (`tools/floor_census.rs`; `floor_census [--cell 0.5] [--tol 0.3] [--png DIR] [map…|all]`)
The world as the sweep builds it (con layer, common merged, `install_collision`); the DRAWN world = every instance's
`resolve_model` mesh at `instance_matrix` (+ the terrain when drawn). Per XZ cell every collision surface on the vertical
line (faces closer than STEP_HEIGHT are one level). **A** = a walkable level with ≥ 1.2 m of room and no drawn triangle
within 0.3 m; **B** = a drawn up-facing non-blend surface with no walkable collision within 0.3 m. Both are kept only
where a flood from the command posts reaches (a step ≤ 0.6 m up, any drop, a thin capsule's wall sweep; B: next to a
reached level). **holes** = reached levels where `Sim::ground_under`, asked from the level, answers > 0.3 m lower.

| map (0.5 m cells) | reachable levels | A pts (m²) | B pts (m²) | holes |
|---|---|---|---|---|
| bf2_kam1 | 665,938 | 420,327 (105,082) | 4,033 (1,008) | 544 |
| bf1_kam1 | 877,377 | 642,880 (160,720) | 24,344 (6,086) | 313 |

BF2 KAM1's A is the disc's own prims, all hidden inside drawn hulls: two soldier-only discs `p_-s-rob` (cyl r 102,
0.71 thick, 7.9 m under each mushroom's floor; 29,071 + 28,935 m²), eleven platform cores `p_-so-rob73` (cyl r 37.3,
h 4; ~4,170 m² each), then the tops of the 2.2 m wall boxes along bridges and ramps (0.44–0.77 m over the drawn rail;
≤ 14.5 m² each). They are reached only through §4.3's slits. BF1 KAM1's A is the y = 0 heightfield (609,483 pts,
152,371 m² — TENS) and hull slopes where the collision cone sits 0.34–0.40 m off the drawn hull (authored
simplification; B lists the same slopes). All 32 maps at 1 m cells: `census/all_maps_cell1.md`; leads there, not
followed: BF1 BES1's heightfield 265,324 m² (TENS), BF1/BF2 END1 ~709,000 m² of undrawn heightfield outside the drawn
terrain, BF2 MUS1 15,160 m² where `mus1_bldg_conference`'s collision floor is 0.5–0.8 m over the drawn
`mus1_bldg_world_1` surface, BF2 MYG1's platform cores, BF2 UTA1's `p_floor` boxes.

### 4.2 The rail is walkable on the PS2, and the hull is walked glued [V-PS2]
`kamr_ps2.py rail`: the soldier 14 m from cp5's pad centre, stick forward 6 s (`out/rail_0.csv`): feet 78.421 on the
deck, **79.106 at r 23.1 (over the 0.57 m rail)**, then down the hull on its surface — r 27.8 → 75.82, 31.1 → 73.58,
34.0 → 71.61, 37.3 → 69.38 (slope 0.68 = 34°, y linear in r: no hop) — off the rim at r ≈ 37.3 and dead. The port's
soldier (`tools/kamr_walk.rs bf2_kam1 21.86,-70 21.65,-45`) also steps over the rail (78.94) but then HOPS: airborne
from t 60, landing every ~18 ticks (STHOP's defect; the PS2 trace is its reference).

### 4.3 The PS2 soldier stands on a slit; the port's falls [V-PS2][V]
The census's B patches in the cloning rooms are slits between collision floor pieces: central chamber (−198.4..−189.4,
58.2..59.2) y 69.29; pod room diagonals at (113.7..116.7, 142.7..147.2) and (93.2..97.7, 165.2..167.7) y 51.5;
corridor and front-entry strips. `kamr_ps2.py drop`: the PS2 soldier released on (−197.35, 58.17) settles at **69.272**
— the control spot 3 m away 69.273. The port there: `ground_under` = 61.409 (the hidden disc); a walking soldier is
airborne one tick and lands on the ramp beyond, a standing or stopping one drops 7.9 m and is under the map.
`core/src/sim/state.rs::ground_under_with` casts one ray at the feet's (x, z) [V]. Second flaw, same query:
`collision/rays.rs::ray_down_with` takes ONE hit per body and drops the body when that face is steeper than walkable,
so a walkable face 0.18 m under a 61° bevel (|n.y| 0.48) at the pads' rim (21.65, −51.83) is never seen and the answer
is the core 9.4 m down [V]. The PS2's foot is not a ray: lane 2's STHOP pass, finished the same evening, reads it as
the sphere `coll_body_lower`, r 0.57 at (0, 0.57, 0.15) (`re-soldier-ground.md`, [O], not re-read here) — wide enough
to bridge the chamber slit, which is what the drop test shows.

## 5. The poses
52 of the 70 pairs viewed at full size (`sbs/`, `sheets/`; port at main 0994d9a). By post, what differs and whose it is:

| posts | difference (measured where a number is given) | cause / card |
|---|---|---|
| every outdoor view, both games | sky 0.47–0.76 × | §1, KSKY |
| every outdoor view with a horizon | PS2 black under the horizon (23,25,27); port a dark-teal still plane (33,41,41) | §2, KSEA |
| every outdoor view | rain streaks, splashes; lightning bolts | WTHR |
| decks, BF2, all four bearings | at 0994d9a PS2 1.5–2.1 × brighter, uniform over RGB ((72,78,86) vs (42,46,50)). **At 5c73167 (BPNT's local lights) they match**: (72,76,81) / (87,92,96) / (86,90,95) vs the PS2's (72,78,86) / (71,79,89) / (83,92,102); the b270 highlight is still short (108–117 vs 122–157) | BPNT (landed); the highlight: GLOWB / SPHOT |
| decks, BF1, all four bearings | PS2 1.5–2 × brighter: (65,79,83) vs (41,46,52); the port's deck does not change with the bearing (41–43), the PS2's does (61–85, 148 at a highlight) | unread [H]: **B1DK** |
| BF2 cp1/cp2/cp4/cp5 | white lamp flares | LFLR |
| far domes and saucers | PS2 lighter and softer ((46,51,56) vs (33,36,39), 1.4 ×); the `_lowrez` sheet is only 15 % brighter (77 vs 67 mean) | far scene [H]: FOGN, LOD |
| BF2 cp0 (chamber), cp3 (pod room) | bluish bloom; pink console tint (the CP's light) | GLOWB, BPNT |
| BF2 cp5 spawn views | the post's blue beam and blue-lit floor ring; the port's pose render draws no hologram | BPNT, LFLR |
| BF2 cp5 b000, cp1 b180 | small gun turrets on the decks that the port does not show there | TBIND (in progress) |
| BF2 cp5 v4/v5 (the lower pad) | two small droids (health / ammo) on the PS2's pad, none in the port frame — LOW, not followed | note on KAMR |
| BF1 cp0 spawn | a black slab fills the port frame: the eye is inside a `kam_prop_pillar`, drawn double-sided | BFC |
| BF1 cp2/cp0 b090 | white mist puffs | DUSTE |
| BF1 cp3 (cloning room) | the white pendant is authored (PS2 shows it); windows bloom more on the PS2 | BRGT |

## 6. Checks (`re/checks.txt`), verifier (`re/VERDICTS.md`)
`re_check.py fn` on every cited line (B2 FUN_0045aa30 / 0045ae80 / 003cb918 / 003bcb08 / 0057ca38 / 005804b0 / 005804d0 /
00575670 / 00574a70; B1 FUN_00377c40 / 00378120 / 002f2308 / 00492598); `writers` DAT_0076c200 1, DAT_0076c220 1,
DAT_007bb7f0 1, DAT_0066164c 2; `callers` FUN_00574a70 2, FUN_00575670 1, FUN_0045ae80 0 (vtable); `grep 0x2855` 4 hits.
One Opus verifier, foreground, claims and citations only: **22 claims — 21 SUPPORTED, 1 PARTLY, 0 NOT SUPPORTED**.
PARTLY (D4): headless steps the water only on its bench and `--cam` paths — worded so in §2.4. Its own finds, each
reproduced here: BF1 YAV2 authors the same Lightning block (dome 153 in its states); four reference states were saved
mid-flash (§1); `DomeInfo.Ambient` has no PS2 reader; the far-ocean branch binds "fogspread"; the first 0.2 s of a level
use an uninitialised brightness (L+0x80).

## 7. For the builds
- **KSKY**: bake the Lightning block; shell 0's colour × (1 − SkyDomeDarkenFactor) on BF1 KAM1, BF1 YAV2, BF2 KAM1.
  Flashes, bolts and rain are WTHR's (the tick's flash arithmetic is in §1).
- **KSEA**: an ocean map draws no water past the far pass (whole 160 m tiles reaching 0.8·FarSceneRange) and clears
  black behind; BF2's far ocean takes the world fog; `pose_render` steps the water.
- **KSEAL**: BF2's ocean lit — PROVISIONAL rule fitted to §2.1's control row.
- **GRND**: the ground query — a foot wider than a point, and a ray that continues past a non-walkable face; the census
  becomes a committed example and its `holes` / reachable-A count the acceptance.

## 8. Still [H] / open
- The grey ellipse of `kam1_play_7` and where the maintainer stood for `kam1_play_8/10/11` (no pose reproduced them).
- BF1 KAM1's sky stays 0.64–0.72 × after the dome factor (BF2 0.94): something else dims BF1's dome or the port's BF1
  sky is brightened (not the BFLT filter alone: off, 0.72). **CLOSED by KSKY** (§1 "Built"): the prototype multiplied in
  linear space; with the linearised factor the band reads 0.96.
- BF1's decks (1.5–2 × dark, card B1DK) and the far objects' lighter look: attributed, not taken apart on the PS2.
- The lit ocean's exact rule; how the far pass culls tiles (frames only); the PS2's foot rule (STHOP).
- bf2_nab2's low sky ratio (0.54); the census leads on END1, MUS1, MYG1, UTA1.

## 9. Addendum 2026-10-01 (Fable, the maintainer's chat session) — what the maintainer's play frames actually were [V port]

§0's rows "invisible floors/platforms" and "the grey ellipse", and the line "No misplaced object was found", stand for the
stored poses and are WRONG for what the maintainer saw in play. `lead_tmp/playtest_0930/kam1_bf2_2230_*.png`, `kam1_play_11` and
`kam1_play_7` are one port defect that no pose render can show:

- `upload_scene` pushes the water's draws first (BF2 KAM1: range 0..2). `WaterMotion::step` on an ocean SPLICES that
  range with the frame's wet cells (128–132 draws), moving every later draw by +126..+130; the count changes with the eye.
- A door's moving submeshes (`DoorDraw.parts`) and the speckle frames kept the draw indices recorded at upload. The battle
  loop steps the water and then calls `pose_doors` every frame, so each door's pose was written onto whatever draw had
  slid under its indices. The victims lose their matrices for the session; the doors never move.
- Probe (upload, one water step from four eyes, pose the shut doors): 48–92 non-water draws moved on main; named by
  original position — `kam_bldg_ramp15` (the maintainer's undrawn walkway), `kam_bldg_ramp17`, `kam_bldg_landplatform16`'s deck (sent
  to `kam_bldg_podroom_door27`: the grey disc in the sky), `kam_bldg_interior_corridor_a`, `right1`, three turrets, pillars.
  `pose_render`, `game --headless --capture-frame` and the windowed fly camera draw every walkway: none poses doors.
- Fixed in ec47e0b (`SceneGpu::animate_water` → `reindex_after_splice`). Evidence `engine/artifacts/lead_tmp/kamdoor/`.

Also since: §4.3's ground query is built (3c7b00c: a ring of rays at 0.3 m inside the foot sphere, and the ray continues
0.5 m under a steep face) — bf2_kam1 holes 544 → 139, none of the 25 largest clusters deeper than 0.47 m; and the bots that
stood beside the walkways (§4.2's rail was not the cause) were the port's hub ARRIVAL rule, f293fd2
(`engine/artifacts/lead_tmp/kamai-fable/README.md`).
