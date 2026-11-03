# OPUS RE (loop lane 3, 2026-09-30) - [O] UNAUDITED
# RE: why lamps, strips, screens and lava glow on the PS2 and not in the port — BF2's self-lit segments and HDR bloom, and both games' DomeInfo Filter (card GLOW)

**The question** (card GLOW; FINDINGS F28, F48, F55, F58, F61, F64; LITE's note): BF2's lamps, window panels, light strips
and lava glow white with a halo on the PS2 and are flat in the port. Which bit or channel marks a surface self-lit, on each
game, and what does the bloom do? Method: the loop's RE discipline (the five self-checks, then one Opus verifier given the
claims only: `lead_tmp/re/glow/claims.md` → the verdicts in §9), backed by the PS2 taken apart on PCSX2's **software**
renderer (patched save states, §6) with the frame, its alpha plane and the bloom's input read straight out of the saved
GS VRAM. Scratch, scripts, frames and sheets: `engine/artifacts/lead_tmp/re/glow/`. B2 = `analysis/SWBFRNT2.ELF-decomp.c`,
B1 = `analysis/SWBFRONT.ELF-decomp.c`. GS ALPHA: Cv = ((A − B)·C >> 7) + D. Tags: [V] read or measured, [D] derived,
[H] guess; load-bearing claims are also [O].

## 0. The answer

At the 2026-09-30 research baseline, three mechanisms were absent from the port.
The current implementations are `render/src/shader.wgsl` (self-lit paths),
`render/src/filter.rs` and `render/src/bloom.rs`; their presence does not establish
original-game acceptance. The original measurements below remain historical evidence.

| effect | where | trigger | what the PS2 does | measured on the PS2 |
|---|---|---|---|---|
| **self-lit glow** | BF2, the 13 maps with `Effect("HDR")` | MTRL **0x10** without 0x04 (245 segments on the port's maps, **all unlit**) | the segment draws with ALPHA 8 = Cs·As/128 + Cs and colour 0xff404040: **× (1 + As/128) ≈ ×2.98** for a texel of disc alpha 255, the texture's **alpha is the glow mask** | MYG1's cyan strips turn white; exact per-pixel match (§1.4) |
| **unscaled translucent** | same maps | MTRL **0x14** (11 segments: screens, shields) | skips HDR's ×0.5, so it lands **×2** after the composite | not isolated (none in view) |
| **HDR bloom** | same maps | `Effect("HDR") Enable(1)` + per-map keys | pixels whose luma in the half-scale frame ≥ GlowThreshold·255 (i.e. ≥ 1.0 on screen for 0.5) are downsampled 4×, blurred by NumBloomPasses×2 five-tap passes, magnified and **added** | MYG1: +16 frame luma; simulated from the PS2's own pre-bloom frame within ~10 % (§2.5) |
| **DomeInfo Filter** | **every BF1 map**, BF2 TAT2 (and TAT3) | sky `DomeInfo { Filter(1) Threshold(T) Intensity(I) }` | frame += I/128 · max(0, frame₂ₓ↓ − T), per channel (a soft lift of everything brighter than T) | BF1 TAT1 **+25 frame luma**, KAS2 +6, BF2 TAT2 +4; model within 1.6–3.9 levels (§3.3) |
| Softness (not a port target) | every map, both games | `DomeInfo Softness(1) SoftnessParam(60)` | lerp(frame, blurred half-res copy, 60/128) | SD blur; mean-preserving |

- **BF1 has no per-segment glow and no HDR** (no 0xff404040, no HDR key, no scale global in B1 — §5). Its lamps get only the Filter.
- **The Filter explains card TATL:** applied to the port's stored TAT1 walk frames it moves the median luma gap (port − PS2)
  from −12.8 to −0.1 over 24 pairs (§3.4).

## 1. BF2's self-lit segments (MTRL 0x10) [V][O]

### 1.1 The override (B2 FUN_0058ed70)
- The material colour of every segment is scaled by *(decompiler detail omitted)* (= DAT_007c5754, the HDR scale, 0.5 when HDR
  is on) — except for UI draws (F & 0x4000000) and segments with **flags & 0x14 == 0x14**, which take 1.0 (723501–723505;
  ELF 0x58f104–0x58f138, `andi v0,v0,0x14` at 0x58f118).
- **When a local < 1.0 and flags & 0x10 and not flags & 0x04:** a local (the pass's GS ALPHA) = **8** and the colour word
  = **0xff404040** (723539–723548; ELF 0x58f344–0x58f370: `bc1f`, `andi 0x10`, `andi 0x4`, `lui v0,0xff40`,
  `addiu fp,zero,8`, `ori 0x4040`, `sw v0,96(sp)`).
- a local is the low byte of the pass's state record (*(decompiler detail omitted)*, 723561 / 723607; ABE set when
  a local ≠ 0, 723557–723559). VU1 sends that record to ALPHA_1 (verifier: 0x4c5–0x4d4). ALPHA 8 = A Cs, B 0, C As, D Cs:
  **Cv = Cs + Cs·As/128**.
- The colour 0xff404040 replaces the computed one, so the caller's colour (instance tint, fade alpha) is dropped for these
  segments; RGB 0x40 equals the normal half-scale material, alpha **0xff** instead of 0x80.
- The HDR scale has 23 readers (terrain, water, sky objects, particles…); only this draw has the 0x10/0x14 exceptions (§9).

### 1.2 The vertex alpha (VU1) [V]
The colour word reaches VU1 via FUN_00497cf0 (command 0x43a, bytes widened to lanes); VU1 0x444–0x446 converts
ITOF0 × 1/128. An unlit segment (0x2a3) writes vf1.xyzw (+65536) as every vertex colour without COL0 (0x2b3–0x2ba), and
multiplies xyzw by COL0 with it (0x2da–0x2ef, clamped at 0xff). So **Av = 0xff**, or 0xff·COL0.a/128 with COL0 — the glow
segments' COL0 alpha is 127 (253) except the lava edges of MUS1's world meshes (0, a fade).
With TFX MODULATE, As = At·Av >> 7 and At = disc alpha·0x81 >> 8 (the `pal_` loader), so **As ≈ disc alpha**: the
**texture's alpha channel is the glow mask**, gain 1 + As/128 from 1 (alpha 0) to 2.98 (alpha 255).

### 1.3 The census (`glow_census.py`, all 17 BF2 port maps) [V]
- 245 segments carry 0x10; **none carries 0x01** (all are unlit). 244 are on the 13 HDR maps (the other: DAG1's X-wing,
  HDR off, so no glow). 234 are 0x10-without-0x04, 11 are 0x14.
- The card's cases are all 0x10: UTA1 `uta1_bldg_hanger_light` (the hangar lamps), MYG1 `myg1_bldg_med_spiral` (the
  corridor strips) and the other spirals, POL1 `pol_bldg_interior_building` / `Pol1_bldg_cavernlights_2` (the skylight),
  MUS1 `mus1_bldg_lava_lava` / `mus1_bldg_world_*` (`mus1_main_2/3`: the lava), COR1's halls and screens, GEO1's bunker
  lights, NAB2's `nab2_prop_glow` windows, KAM1's consoles.
- Most glow textures have alpha 255 everywhere (atlas regions the segment's UVs pick); some carry a real mask
  (`Cor1_Bldg_security_roomDoor` 97 % alpha ≤ 5; `mus1_main_3` mean 178; `nab2_prop_glow` mean 118).
- Three **sky domes** carry 0x10 (GEO1, KAS2, MUS1) with 85–98 % alpha ≤ 5: their few masked texels glow ×3 — a lead for
  card SUNG (BF2 KAS2's sun glare).
- 0x14 (translucent + unscaled): POL1 `pol1_bldg_radar_screens`, `pol1_prop_cavern/hanger_shield`; TAN1's console fx;
  FEL1 `fel1_bldg_segmented` (0x54: additive).
- A 0x10|0x02 segment (MUS1 `lava_hall`, `hallway_ribbed`, `world_*`) is alpha-tested against the overridden As: the
  cutoff falls to disc alpha ≥ 98 (As ≥ 0x60) instead of ≥ 193 [D].

### 1.4 Measured (MYG1 cp4_cp5, the corridor; `ps2exp/sheets/myg1_*`) [V]
The frame read from the SW state's VRAM before the composite (`maskonly_nosoft`, §6), strip pixels only:
- plain (override off) → alpha-only (ALPHA 8 with the normal alpha 0x80) = exactly **2 × plain** (83 → 166, 116 → 232);
- plain → full override = **min(255, plain + plain·253/128)** (83 → 248, model 247; the rest 255). Every glow pixel in
  the frame matches the model; the misses are the animated soldier and snow particles (`myg1_model_fit_nosoft.png`).
- On screen (composite ×2) the strips are white; with the override off they are the port's flat cyan (`bf2_myg1_4.png`).
- With Softness on (the shipped state) the frame is a blurred blend, which spreads and caps the glow: measure the
  override with Softness off.

## 2. BF2's `Effect("HDR")` [V][O]

### 2.1 Keys, defaults, maps
- Object "HDR" (FNV 0xfeb6dbe1, 516638), ctor FUN_00455da8: off, DownSizeFactor 0.25 (+0x430), NumBloomPasses 5 (+0x434),
  MaxTotalWeight 1.2 (+0x438), GlowThreshold 0.5 (+0x43c), GlowFactor 1.0 (+0x440) (516640–516645).
- Parser FUN_00455f30: Enable 0xaf8bb8ce, DownSizeFactor 0x7e6c7187, GlowFactor 0xac9eee97, NumBloomPasses 0xd4ef5803
  (int), MaxTotalWeight 0xeafd07ed, GlowThreshold 0xb12c9b1f; a later key overrides an earlier one.
- Enable → FUN_00456098: registers the pass (FUN_00451678, while DAT_00601284 = 1 in a level) and sets the scale 0.5.
  **Split screen forces it off** (… the reflection render turns it off around itself
  (FUN_00386480, 382827).
- The disc (`hdr_census.py`; RAM confirms on 7 maps): Enable(1) on COR1 DEA1 FEL1 GEO1 HOT1 KAM1 KAS2 MUS1 MYG1 NAB2 POL1
  TAN1 UTA1 (+ the space maps); **not** on DAG1 END1 TAT2 YAV1. Authored values:

| map | NumBloomPasses | MaxTotalWeight | GlowThreshold | DownSizeFactor |
|---|---|---|---|---|
| COR1 GEO1 KAM1 KAS2 MUS1 MYG1 NAB2 POL1 | 5 | 1.2 | 0.5 | 0.25 (defaults / POL1 authored equal) |
| DEA1 / TAN1 | 4 | 1.1 | 0.5 / **0.9** | 0.25 |
| FEL1 / UTA1 | 3 | 1.0 | 0.5 | 0.25 |
| HOT1 (and SPA1–9) | 4 | 1.05 | **0.7** | 0.5 |

### 2.2 The pass (FUN_004665d0 → FUN_0058b188, FUN_0058b9a8)
FUN_004665d0 reads only +0x43c, +0x438, +0x434 (ELF 0x466608–0x466620): **DownSizeFactor and GlowFactor are read by
nothing we found** [V the pass; H elsewhere] — the 4× downsample is fixed (DAT_0060c5fc = 1, no writer).
1. **Threshold** (FUN_0058b188): a 256-entry CLUT, alpha 0x80 for index ≥ int(GlowThreshold·255), else 0 (721651–721660),
   applied per page to a luma-like byte built from the half-scale frame, written into the frame's **alpha** (RGB = Cd,
   721680–721681). The measure, fitted on the PS2's raw masks at 4 thresholds × 4 scenes (§6): **Rec.601 luma agrees on
   99.0 %** of pixels (best free fit (0.375, 0.50, 0.20), 99.1 %); single channels 97.6–98.7 %. The weights the GS
   program appears to build (0.305 R + 0.594 G + 0.695 B) score 93.9 % and are rejected [H: the exact construction].
2. **Downsample** 512×448 → 128×112, one bilinear sample per 4×4 (the central 2×2), ALPHA 0x88 = Cs·As/128 (keeps only
   the masked pixels) (722175, 722185–722210).
3. **Blur**: NumBloomPasses iterations × 2 ping-pong sub-passes (n = 1 … 2·passes, 722212–722339), each 5 bilinear taps at
   (±1, ±1)·n/2 texels and the centre, weight each FIX/128 with **FIX = int(int(MaxTotalWeight·127)·0.2)** (30 for 1.2;
   27 for 1.1, 26 for 1.05, 25 for 1.0); tap 0 overwrites, the rest add; every write clamps at 255 (COLCLAMP 1, the only
   write of register 0x46, 716147).
4. **Composite**: out = frame·255/128 + glow, the 128×112 buffer magnified 4× bilinear (ALPHA 0xff00000029, 722347,
   722359–722375). The ×255/128 undoes the scene's ×0.5, so HDR is level-neutral except where it clips or blooms.

### 2.3 What that means on screen [D]
- Only pixels brighter than **2·GlowThreshold** of display white (1.0 at 0.5; 1.4 on HOT1; 1.8 on TAN1) bloom: in practice
  the glow segments, 0x14 segments and very overbright lit surfaces.
- Per sub-pass gain 5·FIX/128 = 1.17 (1.2), so 5 passes ≈ ×4.8 of energy; spread σ ≈ √(0.8·Σn²/4) ≈ 8.8 buffer texels
  ≈ 35 px at 512 wide ≈ **7.8 % of the screen height** (3 passes: ≈ 4 %).

The Mustafar lava tune drops the fixed tap from 30 to 28 for all ten sub-passes. For
Mustafar's self-lit hallway strips, the port compensates with an input weight of
(30/28)^10 after the bloom threshold. This approximates the authored strip halo using
the same blur kernel. The tuned lava and the strips' base material gain are preserved.
The bloom buffer also carries the visible strips' coverage so the composite can use a
local four-tap approximation of the PS2's 60/128 Softness blend around their edges.

### 2.4 The 0x14 segments [V code, H look]
They draw at full material colour into the half-scale frame, so their translucent contribution is doubled on screen and
they pass the bloom threshold easily. None was in view in the states used.

### 2.5 Measured bloom (MYG1; `simbloom.py`) [V]
From the PS2's own pre-bloom frame (true half-scale values + raw mask, Softness off), the exact 8-bit cascade above
reproduces the PS2's bloom (ctl − no-bloom): energy 19.1 vs 17.5 luma, correlation 0.88, radial profile 2–5 px 66 vs 65,
10–20 px 41 vs 41, 30–45 px 15 vs 12, 45–60 px 5 vs 3. With the glow off nothing on MYG1 passes the threshold: the glow
and the bloom switch together (frame luma: both on 99.7, no glow 82.8, no bloom 83.5, neither 82.8).

## 3. The DomeInfo Filter (both games) [V][O]

### 3.1 Keys and the pass
- BF1 FUN_002e29e0 (the DomeInfo switch): `Filter` 0xc7e16877 → DAT_00522b5c, `Threshold` 0x4939f3f8 → DAT_00522b70,
  `Intensity` 0x8563e50a → DAT_00522b74, `Softness` 0x8958bcda → DAT_00522b78 (270177–270350). They are also console
  variables `Renderer.GlowFilter / GlowThreshold / GlowIntensity / Softness / SoftnessParam` (567847–567858).
- FUN_004a57e0 runs the filter FUN_0049fe60 when DAT_00522b5c ≠ 0 (565446–565458; behind DAT_00522e08, ELF 1, no writer).
- FUN_0049fe60: nothing when Intensity < 1 (563488); else downsample 2:1 to 256×224, subtract Threshold (ALPHA 0x62,
  563529–563540), two small blur iterations (offsets (2, 3), (0, 4) in **1/16 px** after int truncation — sub-pixel), and
  add Intensity/128 of the result (ALPHA 0x68, RGBAQ 0x80808080, 563772–563780).
- BF2's twin: DomeInfo `Filter` → DAT_0060c2c9 (FUN_003bcd50, 416382); FUN_00587278 → FUN_00585890 (718480–718790), the
  same words (Threshold DAT_0060c2d4, Intensity DAT_0060c2d8).

### 3.2 Per map (`key_census.py`; RAM confirms TAT1 NAB1 KAS2 END1 HOT1 and BF2 TAT2) [V]
| game | maps | Filter | Threshold | Intensity |
|---|---|---|---|---|
| BF1 | KAM1 KAS2 NAB1 NAB2 TAT1 | 1 | **50** | 50 |
| BF1 | BES2 | 1 | 100 | 50 |
| BF1 | BES1 END1 GEO1 KAS1 RHN1 RHN2 TAT2 YAV1 YAV2 | 1 | 150 | 50 |
| BF1 | HOT1 | 1 | 0 | 0 (no filter) |
| BF2 | TAT2 (port map), TAT3 | 1 | 150 / 50 | 50 |
| BF2 | COR1 MUS1 (GAL1) | 0 | — | — |
(TEST.LVL: Threshold 90, Intensity 40.)

### 3.3 Measured (patched DAT_00522b5c / … `simfilter.py`) [V]
| state | frame luma on / off | model (from the off frame) mean\|model − on\| | reload noise |
|---|---|---|---|
| BF1 TAT1 cp3_cp2 | 144.5 / 119.5 | 2.7 (+2 bias) | 0.4 |
| BF1 KAS2 cp5_cp10 | 72.1 / 65.7 | 3.9 | 1.2 |
| BF2 TAT2 cp1_cp2 | 154.9 / 150.5 | 1.6 | 0.6 |
The model: out = min(255, v + I/128 · up2×(max(0, box2×(v) − T))). The HUD is drawn after the filter.

### 3.4 The port [D]
At that research baseline, the port had no filter. Applied offline to the port's stored walk frames (09-29) with each map's values: TAT1 −12.8 →
−0.1 median dluma (24 pairs), KAM1 −6.8 → +1.3, NAB2 −5.9 → −0.6, KAS2 −7.7 → −3.7; the threshold-150 maps move ≤ 3;
**NAB1 −7.3 → +10.5 and BES2 −1.0 → +8.8 overshoot** (the port is already too bright in their bright areas — other terms).

## 4. Softness (both games) [V]
Every map of both games authors `Softness(1) SoftnessParam(60)` (SPA 30, FEL1 65, GAL1 0). The pass (B1 FUN_004a21f8,
B2 FUN_00586578) blends the frame with a blurred half-res copy by P/128 (ALPHA 0x64, 564669): a softening at 480 lines,
brightness-neutral in the mean. **Not a port target** (COMPARISON_RULES: sharpness). It does confound pixel measurements
(§1.4, §6).

## 5. The BF1 twin [V][O]
BF1 has no HDR effect (no HDR key hash in B1, no `Effect("HDR")` on any BF1 LVL), no per-segment glow (0xff404040 absent
from B1; BF1's 0x10 is "two floats follow", `re-material-flags.md` §2) and no scaled-material path. A BF1 lamp is an unlit
(or bright) texture plus the map's Filter. Unlit = MTRL 0x01 clear on both games (the port's `LightPath::Unlit`, LITEB).

## 6. The PS2 taken apart [V measured]
`ps2exp/mkstates.py` patches copies of the walkabout states; `run.py` boots each under the **SW renderer** (accurate
blending), waits 8 s, saves; the saved state's `GS.bin` (VRAM at byte 425) holds the frame (TBP 0x1000, 512×448) with its
alpha, read by `gsmem.py`. Variants: noglow (0x58f354 → always skip the override), nounsc (0x58f118 `andi 0x14` → 0),
nobloom (GlowThreshold 4.0), none; maskonly (0x46661c `jal FUN_0058b9a8` → nop: the true half-scale frame + the raw mask);
_tNN thresholds; alphaonly / colouronly (the override's halves); _nosoft (… nofilter.
- Two controls per state; COR1, MUS1 and POL1 moved between loads (droid text, embers, a drifting camera) — POL1's
  skylight left the view, so POL1 and UTA1 are **not measured** here.
- **HW Basic** (the reference library's renderer) matches SW here: MYG1 ctl mean |d| 1.9 (luma 101.2 vs 99.7), TAT1
  1.6 / 1.2 — so the library's glow and filter frames stand (card PXSW).

## 7. Build obligations recorded at the research baseline
All BF2 effects apply only on maps whose `Effect("HDR")` enables it; the per-map numbers are the disc's.
1. **GLOW (build) — the self-lit segments.** Bake: carry per primitive "glow" (BF2 MTRL 0x10 and not 0x04, on an HDR map)
   and "unscaled" (0x14); keep the texture's alpha for glow textures (`bake/src/alpha.rs` strips alpha from textures
   whose draws are all OPAQUE); `Effect("HDR")` (Enable, NumBloomPasses, MaxTotalWeight, GlowThreshold) into the manifest.
   Shader: glow = min(1, unlit · (1 + As/128)) with As = min(255, At·Av/128) (At = A·129 >> 8, Av = 255·COL0.a/128),
   **in display (gamma) units** like the GS; unscaled 0x14 = its blend source × 2; a 0x10|0x02 segment's cutoff = disc
   alpha ≥ 98. The glow ignores the instance colour.
2. **The HDR bloom** — keep the pre-clamp brightness up to 2.0 (e.g. an Rgba16Float scene target or the PS2's half-scale
   convention), threshold on Rec.601 luma ≥ 2·GlowThreshold of display white, then a blur whose radius scales with the
   screen height (§2.3: σ ≈ 7.8 % of the height for 5 passes), gain per the authored MaxTotalWeight, added. Native
   resolution, not 128×112 (COMPARISON_RULES).
3. **The DomeInfo Filter** (new card BFLT) — every BF1 map and BF2 TAT2: out = v + I/128·max(0, low-pass(v) − T) per
   channel in display units, applied to the 3D scene before the HUD, with each map's Filter/Threshold/Intensity.
4. **Split screen** — the PS2 turns HDR off with two players (fill-rate limit); the port keeps glow, bloom and filter in
   every view (reversible call).
5. Softness: not ported.

## 8. What stays [H]
- The exact byte construction of the threshold's luma (§2.2: luma601 fits 99 %; the GS program's reading does not).
- DownSizeFactor / GlowFactor: no reader found outside the parser (HOT1 authors DownSizeFactor 0.5).
- 0x14 segments on screen (none measured). UTA1's lamps and POL1's skylight not re-measured (views drifted / no state);
  the build's acceptance uses the library frames, which HW-vs-SW (§6) supports.
- The filter model's +2 bias on TAT1 (rounding of the 2:1 box and the ×I/128).

## 9. Verification
**Self-checks** (`lead_tmp/re/glow/selfchecks.txt`): `fn` for every cited line (all in the named functions); `word` for
0x58f118, 0x58f354–0x58f36c, 0x46661c, the tap table 0x60c5c0, 0x60c5fc / c620 / c63c, 0x60c2d4 / d8, 0x60c3f4 / f8,
0x639a88, 0x522b5c / b70 / b74 / b78 / c78 / c7c / e14 / e30; `writers` for DAT_007c5754 (2), DAT_0060c5fc / c620 / c63c
(0), DAT_0060c2c9 / d4 / d8 / dc, DAT_00522b5c / b70 / b74 / b78, DAT_00522c78 / e14 / e30 (0); `absent` for 0xff404040,
DAT_007c5754 and the HDR key hashes in B1 (0 hits); `callers` of FUN_00585890, FUN_00586578, FUN_0049fe60,
FUN_004a21f8, FUN_0058b188, FUN_0058b9a8. BF2 twin of the filter read; BF1 twin of the glow searched.
**The verifier** (one Opus agent, claims only): 15 SUPPORTED, 6 PARTLY, 0 NOT SUPPORTED. Folded in: the "HDR" hash line
(516638), the blur/composite line numbers, BF1's outer gate DAT_00522e08 (ELF 1) and the console-variable write path,
the filter offsets truncated to int (3.5 → 3/16 px), BF2's outer gate DAT_0060c318 (ELF 0), 244 (not 234) glow segments on
HDR maps, TEST.LVL's values. It also traced the state record to ALPHA_1 in VU1 (0x4c5–0x4d4) and confirmed by byte scans
that 0xff404040 exists only at ELF 0x58f364 and that COLCLAMP's only address-slot write is 1.
