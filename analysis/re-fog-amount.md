# OPUS RE (2026-09-29) - [O] UNAUDITED
# The PS2's fog amount: a `fogspread` texture lookup at planar depth, blended on the stored bytes — BF1 KAS2's red band (card REDB)

Loop lane 2, card REDB (the maintainer's INBOX note of 2026-09-28). Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1),
`analysis/SWBFRNT2.ELF-decomp.c` (B2); the discs' `CORE.LVL`; PS2 RAM and GS memory in PCSX2 save states; PCSX2 frames
captured for this card at the map golden's own pose. Scratch, tools and every raw output:
`engine/artifacts/lead_tmp/re/redb/` (`checks.txt` = the self-checks verbatim, `CLAIMS.md` = the verifier's brief).
Tags: [V] read at the cited line / measured, [D] derived, [H] hypothesis; load-bearing claims also [O].

## §0 Answer

1. **The red is the port's fog arithmetic, not the PS2's fog.** At the golden's two poses, the port paints BF1 KAS2's far
   land ~3–10× redder than the PS2 does. Three port deviations stack [V][O]:
   - **the blend space:** the port mixes fog in LINEAR light, while the GS lerps the stored 8-bit values. On a black
     surface a 10 % fog gives R ≈ 98 linear-mixed against R ≈ 26 on the GS. This is the dominant term: a mean red error of
     +21.9 levels against the PS2's frame, versus +1.9 for the gamma-space lerp with the same curve (§5.3);
   - **the depth:** the port fogs by radial distance, the PS2 by planar view depth (the fog matrix in RAM, §4). At the
     16:9 panel's edges, radial is up to 1.6× planar, which alone puts the ground panel's line past `near` = 1000 m;
   - **the curve:** the PS2's amount is the alpha of a TEXTURE, `fogspread`, looked up at the fog coordinate (§2–§3), not
     `f` itself.
2. **`fogspread` is a texture, not a VU program** (correcting `re-fog.md` §0.1). Every BF1 and BF2 fog pass binds the
   texture whose name hash is fnv1a("fogspread") = 0x6182a9d0 as its second texture. It is looked up in the same table as
   every other texture, and it is a 64×32 8-bpp texture in `CORE.LVL` on both discs: white, with alpha 1→248. [V][O]
   The TEX0 builder itself hashes "fogspread" a fourth time (0x559098 at B1 561702, `FUN_0049c5c8`) to cache its packet
   [V, verifier-found].
3. **The PS2 itself draws a thin dark-red line at the golden's VANTAGE pose.** This was captured on the PS2 by holding its
   chase camera at that exact pose (§5):
   - under the natural fog the far land is (≤ ~40, ~5, ~3), with 887–1146 red-dominant pixels;
   - under a patched blue fog word the line is blue, and with fog off it is black.
   So "0 red-dominant pixels in the vantage panel" is not the PS2's picture. **The ground panel and all 21 KAS2 PXC poses
   show no fog-made red under the PS2's rule** (emulated, §5.5). [V][O]
4. **The white row** (ground panel, row 119) is the sky dome's hazy bottom band. It shows between the forest backdrop's
   black-faded lower edge (+0.05° at eye height 2.5 m) and the dome's own bottom ring (+0.02°). On the PS2, each `Reflect`
   shell is drawn a second time, mirrored, in the same no-Z sky pass, just before its normal draw. The mirrored
   `kas2_mountains` covers that sliver and paints the dark band the PS2 shows below the horizon (§7). That is card REFL.
   [V code][D geometry]
5. **Far heightfield the PS2 culls** shows in 20 of the 64 golden panels (§6). Only BF1 KAS2 (and 2 px of BF1 GEO1) turns
   it red. The per-map far scene (heightfield culled at `NearSceneRange[2]`, `LOWR` sheet past it) is a separate
   change with its own map-specific risks; this card files it (§6).

## §1 The defect, measured on today's main (dead77b; `redb_probe.rs`, `redb_sweep.rs`)
- Committed `engine/goldens/maps/bf1_kas2.png` (23238e1): **6,035 red-dominant px in the LEFT (vantage) panel** (rows
  160–188, the whole width), and 60 in the right (ground) panel (row 121, cols 92–163). The card named only the right
  panel; the left is 100× larger. [V]
- **Vantage panel** (eye (288.41, 70.59, 240.02), yaw 45°, pitch −18.12°, 70° vertical FOV, 960×540):
  - every red pixel is HEIGHTFIELD (5,204 terrain; 831 terrain behind water) at 1,025–1,781 m radial;
  - fog off, the pixels are (2,2,1); as-is they are (86,2,1).
  - `OPENBF_FAR_CULL=1` leaves 0 red (the black clear shows). Adding `OPENBF_LOWRES=1` gives 6,025 red px: the `LOWR`
    sheet at 1,024–1,800 m, fog-off (4,4,3). **The sheet is as black there as the heightfield.** [V]
- **Ground panel** (eye (94.55, 2.5, 291.55), yaw 45°, pitch −21.25°): 60 heightfield px at 1,033–1,116 m radial, fog-off
  (0,0,0), as-is (49,0,0). With the sheet: 23 px. [V]
- **The sheet texture `kas2`** (baked, 256² RGBA; v = z):
  - rows 0–111 (z < −128, the land south of City Hall) are opaque and near-black, mean (4,5,2);
  - the lake is alpha 0;
  - the island is opaque sand. [V]

## §2 The fog pass binds a texture named `fogspread` (both games)
| hop | B1 | B2 |
|---|---|---|
| name → hash | `… `FUN_00444b88(.., 0x558e20)` 551406 (`FUN_0048b9c0`, far water) and 551045 (`FUN_0048b280`, the ocean: hash passed as the THIRD name, 551047) | `FUN_005393c0(&h, 0x64fc88)` 706800 (`FUN_00572458`, the sheet) |
| hash = FNV-1a over `byte \| 0x20` | `FUN_00444b30` 504365–504380 (basis 0x811c9dc5, prime 0x1000193) | `FUN_00539308` (same body) |
| bind (sheet texture, fogspread, 0, 0) | `FUN_0049eb90` 549415 → `FUN_0049e4b8(9, …)` 562850; model 594618; far water `FUN_0049ec00` 551407 → mode 10 | `FUN_00585310` 706801 → `FUN_00584cb8(8, …)` 718227 |
| each of the 4 names looked up as a TEXTURE | `FUN_0049e4b8` (562574): per name `FUN_0049e480` (per-mode cache) and `FUN_00444998(0x6ef408, 0x800, name)`, the global texture table that the segment draw also uses (`FUN_004cf6f0` 594267) for its texture; then `FUN_0049c5c8(…, tex, …)`, the TEX0 builder | `FUN_00584cb8`: `FUN_00539150(0x7c59d8, 0x800, name)` |
[V][O] Strings: B1 0x558d98 / 0x558e20 / 0x55e348 = "fogspread" (stringrefs 6455, 6458, 6576; `re_check word`); B2
0x64fc88 (stringrefs 5765). `re_check absent`: 0x558d98 at 1 site, 0x558e20 at 2 (`FUN_0048b280`, `FUN_0048b9c0`) and
0x55e348 at 1. All of them are hash-then-bind sites. A fourth string, 0x559098, is hashed inside the TEX0 builder
`FUN_0049c5c8` (561702) to cache the fogspread packet (561707–561709, 562240–562241), which fits it being a texture.
BF2's pass runs only under its fog flag (`FUN_005805b8`, 706771; FogRange near < far, `re-fog.md` §3); BF1's always
runs. [V]
- The sheet's two passes (`FUN_0039c1a8` 549498, packer 388494: +0 u64, the u16 at +0xc into the VU word, +8 TEST).
  **The LIVE words are the branches under `DAT_00522a4c/50/54`, which read like debug toggles but hold 1 in the ELF and in
  RAM, with no writer** (`DAT_00522a48` = 0). This was verifier-found and reproduced: `re_check word`/`writers`, and the
  live record sits in RAM at 0x7b6ac0 (vlevel) / 0x7b3a10 (vantage), while the "plain" record appears nowhere. So:
  - pass 1 = u64 0x800800 / prim flags 0x10 (TME) / TEST 0x5c10b: alpha test GEQUAL 0x10, **DATE = DATM = 1** (drawn only
    where the destination alpha bit is set), Z GEQUAL;
  - pass 2 = **u64 0x800944 (ALPHA byte 0x44) / prim flags 0x50 (TME | ABE) / TEST 0x5c000** (DATE = DATM = 1, Z GEQUAL),
    549452–549494;
  - BF2 hard-codes the same pair (B2 707255–707266: 0x800800 / 0x5c10b / 0x18 and 0x800944 / 0x5c000 / 0x58).
  - The prim-flag reading (0x10 = TME, 0x40 = ABE, 0x08 = IIP) is `re-water-alpha.md` §1.6's for the same record layout.
    In the u64, 0x800800 is VU1 cull enable plus side flip, and 0x100 is the "UV-matrix program" (`re-vu1-cull.md`, whose VU
    reading is agent-only) — the fog matrix feeding the texture coordinates. [V words, D/H bits]
  - So the fog pass is **textured**, with texture 2 = `fogspread`, and blended as (Cs − Cd)·As + Cd. [V][O]
  - The DATE mask means the sheet draws only where something earlier in the far target left alpha ≥ 0x80. After the
    RGBA-0 clear, that is the sky pass, whose `FB_ONLY` writes the shells' own alpha. On BF1 KAS1/KAS2 the mirrored
    `Reflect` dome (§7) covers the whole lower hemisphere, so the far land is drawn [D]. This is an input to card FARS, not
    to the fog amount.
- The TEX0 path is the engine's one texture path: MODULATE with TCC = 1 for a paletted texture (`re-water-alpha.md` §1.4
  [O]). So As = At(fogspread)·Av » 7 and Cs = Ct·Cv » 7. Here Cv is the fog colour halved (549500–549502:
  `(c + 1) >> 1`, so 255 becomes 128 = GS unity), and Ct is white. [V code, D]

## §3 The texture, on the disc and in GS memory
- `CORE.LVL` on both discs (`redb_texdump`, a scratch mungefs example, and the verifier's own chunk walk):
  - the `tex_` headers are at file offsets **0x104534** (B1) / **0x2ae0bc** (B2). The redb_texdump printout's
    0x10452c/0x2ae0b4 are ucfb-payload offsets, 8 lower [V, verifier-corrected];
  - `fogspread` is 64×32, 8 bpp, one mip, 256-colour palette; every texel is RGB (255,255,255);
  - alpha runs along both axes: row 0 goes 1 → 206 left to right, row 31 goes 38 → 248, column 0 goes 1 → 38, column 63
    goes 206 → 248;
  - the two discs' texel alphas are identical. `fogspread_{bf1,bf2}_dump.txt`, `fogspread_*_64x32.rgba`. [V][O]
- **GS memory** (`GS.bin` of the kept state `ps2exp/states/redb_vantage.p2s`, file offset 0x3951a9):
  - a white 32-bit CLUT whose alphas are the disc palette's halved to GS scale ((a·0x81)»8): 0,1,2,3,4,5,6,7,8,9,10,12,13,…;
  - they sit in the PSMCT32 block order [0,1,8,9,2,3,10,12,4,5,13,14,6,7,16,17 | 46,48,61,63,…];
  - so the GS samples the disc's table unmodified. [V][O] (The verifier's own decode: all 256 entries = (255,255,255,
    (a·0x81)»8) under the CSM1 swizzle; 8 entries are 0 and unused; three more copies sit in VRAM. The cached fogspread
    TEX0 in RAM (0x6cfaf0) is PSMT8 64×32 with CLUT pointer 0x3950.)
- Only this one texture reaches every fog pass: no other name is hashed at the fog sites (§2). [V] In RAM,
  `DAT_007015c0` and `DAT_0069f680` hold 0x6182a9d0 in all three kept states [V, verifier].

## §4 The fog coordinate is planar view depth, and it feeds both texture coordinates
EE RAM of the kept state `ps2exp/states/redb_vlevel.p2s`:
- The fog matrix at 0x6ea8b0 has columns 0 and 1 **identical**: (−3.50148e-4, −4.49675e-5, −3.56846e-4, −0.312150) by
  world (x, y, z, 1).
- That is `fwd/(far − near)` and `(−fwd·eye − near)/(far − near)` for the live camera: fwd = (−0.69756, −0.08958,
  −0.71090), eye (288.413, 70.587, 240.019), near 1000, far 2992.1875. Columns 2 and 3 are (0, 0, 0, 1).
- So per vertex the pass gets (s, t, ·, ·) = (f, f, 1, 1), with f = (planar view depth − near)/(far − near). [V][O]
- The rows 0x6ea980..0x6ea9bf hold lanes 0 and 1 of rows 2–3 only, as KASF found (`re-fog.md` §9.3).
- **How the VU turns (f, f) into the GS's (s, t, q) was not read** [H]. §5.4 shows it is not a plain lookup at (f, f).

## §5 The PS2 at the golden's own pose (PCSX2, `ps2exp/`)
### 5.1 Method (`teleport_probe.py`, `velocity_probe.py`, `vantage_capture*.py`, all ONE foreground scripted run under ci.lock)
- The kept Beach Head spawn state is booted. The player's world-matrix position (health component C + 0x228) is rewritten
  every 2 ms, and the velocity at C + 0xa18..0xa23 is zeroed (0 → −13.8 m/s in a 0.8 s fall, `velocity_probe.py`).
- The chase camera follows. Yaw and pitch come from right-stick holds read back from RAM (ps2live). The soldier target is
  then corrected so the EYE lands on the pose.
- Fog variants are live writes of the words the EE re-reads every frame: colour 0x6ea970, rows 0x6ea9a0/9a4 and
  0x6ea9b0/9b4, near/far 0x6ef3dc/e0 (KASF's `mkstates.py` words). Each variant is shot beside its own fog-off shot.
- Poses (`frames/*_pose.json`):
  - `vantage`: eye (288.413, 70.586, 240.019), yaw 44.46°, pitch −18.04° (golden: 45.0°, −18.12°);
  - `vlevel`: the same eye pitched to −5.14°, so the far land sits clear of the HUD;
  - PS2 FOV 48.0° × 37.69°. All 14 shots of a run held the eye within 1.8 cm.
- Registration to the port's render (805×617 at the PS2's own aspect 1.3045, stretched to the 823×617 game box): PS2
  (x, y) ↔ port (x·805/823 − 1.5, y − 1.5) by gradient correlation, the same offset at all three poses (corr 0.53–0.62).
  [V]

### 5.2 What the PS2 draws there [V][O]
`frames/vantage_4.png`, `frames/vantage_band_x3.png`, `vantage_ps2_port_rule_band_x2.png`, `ps2exp/frames/vlevel_nat_game.png`.
- Under the natural fog the far land south of the lake is a **dark-red line** under the forest backdrop.
  - At the golden pose, the red pixels sit at game-box rows 55–67, cols 178–553; cols 0–150 are behind the ammo HUD
    [verifier-corrected]. Rows 52–68, cols 225–344 average (27.6, 13.4, 7.4), max R 48, under the HUD's translucent
    top bar. At the nat red pixels the three variants read nat (35,8,5), blue (4.5,8,35), off (4.6,8,4.8) [verifier].
  - HUD-free (`vlevel`), on 4,430 registered sheet pixels, the median R is 8 at 1,100–1,200 m planar, 13 at 1,200–1,300 m
    and 35 at 1,400–1,500 m.
- Red-dominant pixels (r > 30, r > 2.5·max(g,b)) in the game box: **887** (`vantage`) and **1,146** (`vlevel`). The same
  frames with fog off have 0.
- With a blue fog word the line is dark blue (B ≤ 33). With the rows zeroed it is black (2–4, 4–7, 1–3), the sheet's own
  colour.
- The port at the same pose paints the band (100, 11.6, 6.9), max R 157 (`vdepth/asis_asis.png`).

### 5.3 The blend space [V measurement, D mechanism][O]
The emulation takes the PS2's own fog-off frame and the port's planar depth for each HUD-free sheet pixel, then predicts
the PS2's natural frame (`natcheck.py vlevel`):

| candidate | mean R error | median R at 1,100–1,200 / 1,200–1,300 / 1,400–1,500 m |
|---|---|---|
| **table diagonal, lerp on the 8-bit values** | **+1.88** | 11.5 / 21.0 / 49.8 |
| linear f, lerp on the 8-bit values | +3.28 | 20.1 / 32.6 / 64.4 |
| table diagonal, mix in linear light | +21.88 | 57.9 / 77.0 / 117.4 |
| the port today (its own render) | +30.33 | 93 / 109 / 132 |
| **PS2** | 0 | **8 / 13 / 35** |

The GS blends the framebuffer's stored bytes, and the port's sRGB-encoded values are what the PXC pairs compare with the
PS2's bytes. So the faithful fog is a lerp on sRGB-encoded values [D]. That term alone is ~20 of the port's 30 levels.
**Caveat (verifier):** 3,067 of the 4,430 pixels lie nearer than `near`, so the diagonal rule's +1.88 is flattered.
In the FOGGED bins it over-predicts the PS2's amount ×1.6–2.1 at vlevel (f 0.10–0.15: PS2 0.045 against 0.075) and
×1.0–1.6 at vantage. A no-fog predictor scores |err| 1.82 against its 1.89 at vlevel. So the table shows the blend
space and the depth are right (~10× closer than the linear-light mix or the port). It does not show the curve is right;
that is §5.4.

### 5.4 The curve: near the table's diagonal on the map's own ramp, not a function of f alone [V][H]
White fog words on patched ramps give A = (C_white − C_off)/(255 − C_off) per pixel, against the port's planar depth
(`fogcurve.py`, tables in `fogcurve_*.txt`).
- On the natural ramp (f ≤ 0.2 in view), A tracks the table's diagonal LUT(f, f) closely at f ≤ 0.1. It runs up to ~35 %
  below it towards f = 0.2 (vlevel: 0.005 / 0.028 / 0.045 at f = 0.025 / 0.076 / 0.121, against 0.013 / 0.042 / 0.074).
- On patched ramps, the same pixels give **different A at the same f** for different ramps (vlevel, f ≈ 0.475):
  - 0.36 on 900..1700 m, 0.55 on 650..1450 m, 0.53 on 0..1500 m;
  - on short ramps there is fog before `near` (0.012 at f = −0.025 on 900..1700 m);
  - on 650..1450 m, A saturates near 0.72.
- No sampling LUT(a·f, b·f) or LUT(f, t(depth)) fits every ramp (`fogcurve_bydepth`, the t-solve).
- The VU program's fog coordinate is therefore not a plain per-pixel (f, f) [H]. This is **card FOGC**: read the VU1 fog
  path, or patch the GS CLUT to an index ramp and read the sampled texel back.
- For the build: on the natural ramp, the table diagonal at planar f, lerped on the stored bytes, over-predicts the
  fogged bins ×1.6–2.1. On the dark land that is at most ~15 levels of R at the far edge, against the port's +60–100
  today. That is PROVISIONAL; card FOGC.

### 5.5 The rule, emulated at every pose that matters [V port renders + D emulation][O]
`emulate_ps2_fog.py`, `pxc_emulate.py`: the port's fog-off render plus planar depth, the table diagonal, and a lerp on the
stored bytes.
- **Golden ground panel: 0 red-dominant px** with the heightfield as today, and 0 with the sheet.
- **Golden vantage panel: 618 px** (heightfield) / 694 (sheet), rows 160–165, mean (45, 1–4, 0–2): the thin line the PS2
  draws. With the maintainer's `FOG_DENSITY_SCALE` 0.92: 584 / 569.
- **All 21 KAS2 PXC poses: 0 fog-made red** (red-dominant with the fog but not without it). The exception is 4 near
  pixels of dark wood at 11–18 m, where the table's 1/255 floor at f ≤ 0 tips (31,13,5) over the threshold. The PS2
  never fogs its near scene, so the rule takes **A = 0 at f ≤ 0**. The port today makes 21 (Beach Head `v2`) and 62
  (Dock Two spawn) there.

## §6 Far heightfield the PS2 culls — the sweep over all 32 goldens (`sweep_table_t1.tsv`, `redb_sweep.rs`)
Method: per panel, at the goldens' PS2 texture tier (`sweep_table_t1.tsv`; its 32 vantage renders are byte-identical
to the goldens' left halves, and its ground renders differ only by the reference soldier it does not draw), the
pixels that change when ONLY the base terrain and its TALPH layers are culled at `NearSceneRange[2]`,
where the PS2's heightfield ends (`re-lowr.md` §0.2 [V]). Also the red-dominant pixels among them.
- **Red-dominant px on far heightfield: BF1 KAS2 (6,035 + 60) and BF1 GEO1 (2 + 0) only.** Every other map's
  red-dominant pixels are content that the cull does not touch: BF2 MUS1 lava, BF2 GEO1 rock, BF2 DEA1 lights, etc. (`sweep_table_t1.tsv` column red_asis)
  [V]
- **Far heightfield visible** (vantage / ground px):
  - with a drawn sheet: BF1 BES2 59,138/0, GEO1 168,195/0, HOT1 154,148/3,365, KAS1 146,250/124, KAS2 305,994/32,729,
    NAB1 310,347/70, NAB2 19,951/0, YAV2 44,177/0, tat1 269,317/13,148; BF2 GEO1 149,300/53,537, HOT1 168,299/3,327,
    KAS2 159,003/0, POL1 58,822/0;
  - with no drawn sheet: BF1 END1 96,875/643, RHN1 106,881/4,131, RHN2 39,878/0, YAV1 225,940/22,262; tat2 (BF2 Mos
    Eisley) 58,028/0. That is FARC's domain.
  - Zero in both panels: BF1 BES1, KAM1 (`TerrainEnable 0`); BF2 COR1, DAG1, DEA1, END1, FEL1, KAM1, MUS1, MYG1, NAB2,
    TAN1, UTA1, YAV1. [V]
- Cull + sheet on the 13 sheet maps (`sweep_{vantage,ground}_asis_vs_sheet.png`, viewed). Most look like a coarser copy of
  the same ground. Two show problems:
  - **BF1 KAS1**: its sheet is ~95 % holes (`re-lowr.md` §5, CLN5), so black clear shows where the PS2 shows its far water
    and mirrored shells (FARC + REFL);
  - **BF2 POL1**: a dark shape near the landing pad.
  So the per-map far scene is ruled map by map in **card FARS**, with deps REDB, FARC for KAS1 and REFL. It is not part of
  this fix.

## §7 The white row: the dome's haze sliver, and the PS2's mirrored `Reflect` shells [V code][D geometry][O]
- The port's ground panel, row 119, cols 0–160 is (96,104,112) and is drawn with zero world draws. The bottom rows of
  `kas2_sky` (v → 1) are haze (158–166, 165–172, 176–181). `kas2_mountains` fades to (0.8,1.4,0.9) at its bottom row, with
  alpha 241.
- `glbinfo.py` gives the dome's lowest ring at +0.02° and the mountains' at +0.20° about the origin. With the ELF's Y lerp
  (Offset 1.0, MovementScale 0.995) at eye height 2.5, the mountains' edge sits at +0.05°. The dome's haze shows in
  between.
- **B1 sky tick `FUN_002f2308`**, per `DomeModel` slot (+0x24 + 0x14·i):
  - when `Reflect` (+0x34) is set (280242), the shell is drawn TWICE with the matrix a local: first with flags
    0x1000031 (280253), then 0x20031 (280255), BEFORE its normal 0x20031 draw (slot 0: 280282–280284);
  - that matrix is row 0 = X, row 1 = −(0.85 + `DAT_00518b24`·`FUN_00434218`(·))·Y, row 2 = Z (280190–280218; the 0.85 at 280212);
  - row 3 = the lerped position + (0, 0.1, 0) (`0x3dcccccd` = 0.1, 280243–280246);
  - `DAT_00518b24` = 0.006 (ELF, no writer). `FUN_00434218` is sinf (π/4 range reduction, 493515). The phase
    `DAT_00518b1c` advances by `DAT_00518b20`·(1/30) per tick; `DAT_00518b20` = 2.0 (ELF) and 280194 is its only writer
    [V]. The argument passed to sinf is not shown by Ghidra [D].
- Sky draws take no Z write under flag 0x20 (B1 572592–572596 in `FUN_004b0c48`, `re-sky-pass.md` §2), so the sky pass is painter's
  order. The mirrored `kas2_mountains` is drawn after the normal dome, and its mirrored lower rows hang from about +1.7°
  down to about −12°. The normal mountains, drawn last, cover it above +0.05°. **Below +0.05° the mirrored dark forest
  covers the dome's haze sliver and the gap under it.** [D]
- That matches the PS2's frames: a dark band right under the backdrop at every KAS2 sea-facing pose, median (10,20,11)
  (`re-fog.md` §9.3), and no bright row.
- The 0x1000031 draw goes down the reflection branch of `FUN_004cf6f0` (594262). Its effect is not read [H]. BF2 KAS2
  authors no `Reflect`. **Read 2026-09-29 (card REFL, `analysis/re-sky-reflect.md` §3–§4 [O]):** bucket 3, drawn
  into the MAIN target before the terrain, with no Z write — the near lake's reflection. The 0x20031 copy is the far
  target's, and it is the one that covers this white row.
- **This is card REFL's build**: the mirrored 0x20031 shells for `Reflect` domes on BF1 KAS1/KAS2. It fixes the white row
  (REDB acceptance item 2 moves there).

## §8 Port rule (PROVISIONAL; the build card REDB)
```
fog amount  A = fogspread_diagonal(f), f = (planar view depth − near) / (far − near)      (far' = the maintainer's 0.92 span as now)
            fogspread_diagonal(f) = the disc table's alpha/255 at normalized (f, f), bilinear, clamped;  A = 0 for f <= 0
blend       out = lerp(encode(c), encode(fog colour), A) on the sRGB-ENCODED values, then decoded for the target
            (the GS lerps the stored bytes; §5.3)
which draws every draw the port fogs today (which ones the PS2 fogs is FOGB / FOGN); the water's own fog gate unchanged
table       bake `fogspread` from CORE.LVL (both discs, identical) into the common bake, or its 64-entry diagonal as a
            constant table cited to this doc
sky         never fogged (unchanged)
```
The curve is the weakest part: in the fogged bins the PS2 sits at 0.5–0.65× the diagonal (§5.3). Keep the table
swappable, and take the captured PS2 frames (`ps2exp/frames/{vantage,vlevel}__nat_r1.png` with their poses) as the build's
reference. FOGC refines the curve.
Expected: KAS2's ground panel 0 red and the vantage panel's thin line (~600 px, mean ≈ (45, 2, 1)). The fog on every
fogged map gets weaker, because the linear-light mix was the dominant term. The 16 fogged goldens move: BF1 BES1, BES2,
END1, GEO1, HOT1, KAM1, KAS1, KAS2, NAB1, NAB2, YAV1, YAV2, tat1; BF2 HOT1, KAM1, MYG1 (the build confirms the list from
its first gates run). The maintainer's `FOG_DENSITY_SCALE` stays and is flagged for him, since it was asked for against the
stronger linear-light fog.

## §9 Checks run (verbatim in `checks.txt`)
1. `re_check fn`: all B1/B2 lines above. 549414/549415/549474/549483 are in `FUN_00488268`; 594618 is in `FUN_004d0108`;
   594788/594789 are in `FUN_004d0a08`; 551406/551407 are in `FUN_0048b9c0`; 280243/280253/280255/280282 are in
   `FUN_002f2308`; 504373 is in `FUN_00444b30`. The B2 bind lines are 706800/706801 in `FUN_00572458` (checks.txt's first
   B2 pair was mis-cited and is corrected here). The live-branch globals: `re_check word` 0x522a48/4c/50/54 = 0/1/1/1,
   and `writers` finds none.
2. `re_check absent`: the three B1 "fogspread" string addresses → 1 / 2 / 1 sites, all hash-then-bind. The literal
   "fogspread" appears in neither decomp (only as string addresses).
3. The twin: B2's `FUN_00572458` → `FUN_005393c0` (FNV) → `FUN_00585310` → `FUN_00584cb8`, which looks up names in the
   texture table. `callers FUN_0049eb90` gives 3 sites: the sheet, `FUN_004b1a20`, and the model segment.
4. `re_check word`: 0x558d98 / 0x55e348 / B2 0x64fc88 = "fogspread"; 0x518b24 = 0.006; 0x518b20 = 2.0.
5. `re_check writers`: `DAT_00518b24` 0; `DAT_007015c0` 1 (594789, the hash); `DAT_00518b1c` 1 (280194, the phase).
- Stayed [H]:
  - the VU fog coordinate path, and why the fogged bins sit at 0.5–0.65× the diagonal (§5.3, §5.4 → FOGC);
  - the effect of the DATE mask and the 0x800800 cull on the sheet (§2 → FARS);
  - the 0x1000031 reflection draw (§7 → REFL);
  - the argument of the mirror's sinf;
  - whether the 16-bit far target's quantisation matters (it is below the measurement noise here).

## §10 Verifier
One foreground Opus verifier on C1–C17 (`CLAIMS.md`), given the claims and citations only. It worked from the decomps, the
ELF bytes and EE disassembly, its own walk of `CORE.LVL`, and EE RAM / GS VRAM from the three kept states. Its scripts are
in `verify/`. **11 SUPPORTED, 3 SUPPORTED with a caveat (C10, C15, C17), 2 PARTLY (C4, C11), 1 NOT SUPPORTED (C6).**
Everything is folded in above:
- **C6, NOT SUPPORTED:** the sheet's pass words were read from the non-debug branch. The live branch (globals 1 in the ELF
  and RAM, no writer) is {0x800800, TEST 0x5c10b} / {0x800944, TEST 0x5c000}: destination-alpha test on, VU cull flags,
  and the UV-matrix program on the fog pass. §2 is corrected. The fog-amount conclusions do not change (same blend,
  same texture), but the sheet's DATE mask and one-sidedness are FARS inputs. Reproduced by the lead (`re_check`
  word/writers; the live record found in RAM, the plain one absent).
- **C4, PARTLY:** there is a fourth "fogspread" hash, in the TEX0 builder (561702), and the ocean passes it as the third
  name (551047). §2 is corrected.
- **C11, PARTLY:** the `tex_` file offsets are 0x104534 / 0x2ae0bc. §3 is corrected.
- **C15, caveat:** the diagonal over-predicts the fogged bins ×1.6–2.1 (vlevel). §5.3 is corrected, and the curve is
  left to FOGC.
- **C17, caveat:** the first sweep ran at the PC texture tier. It was re-run at the PS2 tier (`sweep_table_t1.tsv`): the
  vantage renders are byte-identical to the goldens, and the conclusion is unchanged. §6 is updated.
- **C10 scope:** `FUN_004b0c48` is the Scroll-class draw. The Normal-class draw (`FUN_004cf6f0`) also gives 0x51001 under
  flag 0x20, except when seg+0x2c == 1, where it gives 0x5100b. The mirror argument (§7) is unaffected unless a
  KAS1/KAS2 shell segment has +0x2c == 1 [H, for REFL].
- **C5 omission:** BF2's gate is noted in §2.
