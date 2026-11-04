# OPUS RE (loop lead, 2026-09-25, lane 1) - [O] UNAUDITED: no Fable pass
# RE: the flat water's colour and transparency — the VU1 overlay's routine B, the speckle lobe, the ocean's white (queue WCOL)

**The question** (queue WCOL, PCSX2 FINDINGS F1, F8, F13, F18, F25): on the PS2 the flat water reads as a light teal,
see-through body (BF1 KAS2, YAV1, NAB2; BF2 FEL1, YAV1); in the port it is a near-neutral darkening with a white mottle.
Which term of the draw differs? Method: `engine/artifacts/briefs/OPUS_LOOP_PROMPT.md` §5 — the hops below, the five
self-checks, ONE Opus verifier given the claim list only (§7) — plus two measurements the entry asked for: the PS2's own
draw taken apart in PCSX2 (patched save states, §2) and the port's draw taken apart at the same poses (a scratch
instrument, §3). Scratch, scripts, frames and the verifier's files: `engine/artifacts/lead_tmp/re/wcol/`
(`claims.md`, `verify/VERDICTS.md`, `ps2exp/`, `renders/`, `sheets/wcol_<map>.png`).
Conventions: BF1 = `analysis/SWBFRONT.ELF-decomp.c`, BF2 = `analysis/SWBFRNT2.ELF-decomp.c`; ELF file offset = va −
0x15f000 (both); VU1 instructions decoded with the fresh `scripts/tools/vu1dis.py` (the verifier's own decoder
agrees on all 126 overlay instructions); VU1 relative branches wrap in the 11-bit PC; "GS units" = the colour byte the GS
reads, 0x80 = 1.0 in MODULATE and in the alpha blend. Tags [V] read / measured, [D] derived, [H] guess; every
load-bearing claim is also [O].

## 0. The answer

The flat water's MAIN layer is coloured per vertex by a 63-instruction VU1 overlay the EE uploads every frame, not by
the scene's lights. Its routine B computes, per vertex, in GS units [V][O]:

    colour = LUT[i].rgb + SpecularColor.rgb/255 · F        alpha = ALPHA[phase] + SpecularColor.a/255 · F
    F = (1 − max(0, V̂.y))³,   V = (eye − 10·ĥ) − p,   ĥ = the light's horizontal direction,   both clamped at 255/128

— a Fresnel-like term that makes the water brighter AND more opaque toward grazing view. `SpecularColor` is authored on
every water map (192,192,192 · 192/152/102 on most; BF2 FEL1 192,222,192 · 152; BF2 KAS2 60 · 152; BF2 DAG1 0 · 192).
Then the GS: `Cs = texel × colour`, `As = texel.a × alpha`, source-over. No light colour, no normal (§1.2).

| term | PS2 (this RE) | port today | measured effect (§2, §3) |
|---|---|---|---|
| view-angle term (`SpecularColor`·F into colour AND alpha) | yes, routine B | **none** | removing it on the PS2 takes +13..+27 G/B levels out of the water on KAS2, NAB2, FEL1 — it IS the teal body |
| LUT colour | uniform noise over entries 5..15 = Min … 2·Max − Min, mean ≈ **MaxDiffuse** | flat midpoint of Min and Max (≈ 0.6× the mean) | +0.5..+3 levels alone (the water's own colour is dark) |
| scene lighting on the water | **none** (unlit) | × (ambient + sun·N·L) | < 1 level at these poses |
| speckle | `SpeckleAmbient + SpeckleSpecular · lobe³²`, lobe = horizontal cos toward the sun's azimuth | `SpeckleSpecularColor` flat over the whole surface | the PS2's speckle adds ≤ 5 levels here, the port's +9..+18 — the port's white mottle |
| ocean base (KAM1) | white halved = unity, unlit | the flat-water LUT midpoint, lit | [D] the port's KAM1 sea ≈ 0.3× too dark (F18) |
| alpha table | Min.a..Max.a triangle wave per vertex phase, mean = midpoint | the midpoint | none (same mean) |

**Not a water term, and the larger residual on three maps: the BED under the water.** With the water's alpha zeroed
on the PS2 the channel bed of BF1 KAS2 is pale sand (107,96,77) where the port's is dark grass (41,43,24) (the KTEX F6
family); BF1 NAB2's and BF2 FEL1's beds are 2–2.5× BRIGHTER in the port than on the PS2; BF1 YAV1's floor is greener on
the PS2. The water fix alone cannot close those.

**A side finding for every lit map (§4): the port's sun direction is not the PS2's.** BF1: the port bakes
(−L.z, L.y, L.x) of the PS2's own vector (a 90° azimuth rotation); BF2: (L.x, L.y, −L.z). [V] disc + RAM + code; [H] that
the PS2's diffuse lighting uses the same vector. Queued as SUND.

## 1. The hops [V][O]

### 1.1 The overlay (verifier claim 1, SUPPORTED)
- BF1 `FUN_0048c4f0` (header 551525) line 551546 *(decompiler detail omitted)*; `FUN_0039cb38` writes DMA tag 0x50000000
  (CALL) to its argument. ELF words at va 0x515410 (off 0x3b6410): `60000020 00000000 11000000 4a3f074e` = DMA RET qwc 32;
  VIF FLUSH; VIF MPG 63 instructions at VU1 0x74e. After the tiles, `FUN_0048b9c0` (header 551182) line 551518
  *(decompiler detail omitted)* puts back the 168-instruction default at 0x74e (`60000054 … 4aa8074e`; also uploaded by
  `FUN_0049bf50`, and resident in the YAV1 state's `vu1MicroMem.bin`).
- BF2 `FUN_00576d28` (header 708971) line 708992 *(decompiler detail omitted)*: `60000020 00000000 11000000 4a3f0758`,
  MPG 63 at 0x758. The two overlays are word-identical except pairs 24 and 49, the return address `IADDIU vi10, vi0,
  0x173` / `0x192` (the verifier: both reach the same code for the water's records).
- `.DVP.ovlytab` lists it as overlay 9 (lma 0x515420).

### 1.2 Routine B — the main layer's colour (BF1 VU 0x76c–0x784, BF2 0x776–0x78e; claim 2, PARTLY → corrected here)
`0x76d ADDw.y vf2,vf0,vf0.w` (1), `0x76e SUBw.x vf2,vf0,vf0.w` (−1), `0x76f LQ vf4,148(vi0)`, `0x770 LOI 0x478000ff`;
per vertex `0x773 LQI vf7,(vi11++) | MULAx.xyz ACC,vf6,vf2.x`, `0x774 ILWR.x vi4,(vi9) | MSUBw.xyz vf7,vf7,vf0.w`
(vf7 = −(p + VU[146].xyz)), `0x775–0x777 ERLENG/WAITP/MFP`, `0x778 ILWR.y vi5,(vi9) | MULx.y` (V̂.y), `0x779 LQ.xyz
vf3,151(vi4) | MAX.y`, `0x77a LQ.w vf3,151(vi5) | SUB.y` (1 − c), `0x77b–0x77c` cube, `0x77d MULAw ACC,vf3,vf0.w`,
`0x77e MADDy vf9,vf4,vf8.y`, `0x77f MINIi`, `0x782 SQ vf9,-2(vi12)` into the vertex's colour slot. It reads VU[2].w,
VU[4].w, VU[146], VU[148], VU[151..166] and its two streams — **no light colour and no normal**; the light DIRECTION
enters only through VU[146] = eye − 10·ĥ, the shifted view origin (the verifier's correction of "no light direction").
The projection stage after it (BF2 0x36a) reads the XYZ and ST slots and never the colour slot [V]. [D] the stream at
VU[2].w is the world position: VU[146] carries −eye in world units and the result is normalised as a view vector.

### 1.3 Which layer runs which routine (claim 3, PARTLY → corrected here)
`FUN_0048b9c0` 551326–551327 *(decompiler detail omitted)* / `(…,0xffffffffffffffcf,0)` before the main tiles;
551446–551447 and 551477–551478 (`…,~0,0x40` / `…,0xffffffffffffffc7,0`) before the far and near speckle.
`FUN_0039bc48` sends command {0x425, mask & ~bits, bits, 0}; the VU1 packet loop (0x006–0x00c: `XTOP`, `LQI`, `MTIR vi1,
vf1.x`, `JR vi1`) runs handler 0x425, which keeps the flags in VU[7].x and stores (flags & 0x78) >> 2 in VU[81].w
(0x425–0x435, the 2^25 float trick). The dispatch 0x16a–0x171 jumps to VU[81].w + 0x1d **when bit 0x40 of the draw
record's own gate word is set** (vi8 = VU[8k+6].w, loaded at 0x15d, written by command 0x404 from word 0 of the record
`FUN_0039c1a8` binds: 0x69c3a0/390/380 main, 0x69c3b0 speckle — 0x4144 / 0x4148 in the states, high byte 0x41; the
verifier's correction: the 0x40 test is not on the VU[7].x flags). Table entry 0x2f (flags 0x48) is `B` to 0x76c and 0x2d
(0x40) to 0x74e once the decoded −0x94 / −0xb2 wrap. BF2: `FUN_00575fd8` 708738–708739 (main), 708891–708892 /
708924–708925 (speckles), handler 0x427 → VU[84].w, dispatch 0x174–0x17f, entries → 0x776 / 0x758, 9-qword blocks.
The far speckle's third block (the `fogspread` record, word 0x144) takes the standard path, not the overlay.

### 1.4 The constant block (claim 4, SUPPORTED)
VU[144..166] = the 23 qwords `FUN_0048c4f0` builds at 0x754a30 and uploads with *(decompiler detail omitted)*
(551622); BF2 *(decompiler detail omitted)* (709066).
- q0 VU[144] = (b4, −b8, b4, l̂.x) with (b4, b8) = `LightAzimAndElev` (key 0x9a4f3cc1 → DAT_005649b4/b8, `FUN_00489c40`
  550348) and l̂ = normalize(−L.x·b4, −L.y·b8, −L.z·b4); q1 VU[145] = (ĥx·SCS.x − eye.x, −eye.y, ĥz·SCS.y − eye.z, l̂.y)
  with SCS = `SpeckleCoordShift` (0x6ee80757 → DAT_00564988/5649a0, 550380); q2 VU[146] = (ĥx·10 − eye.x, −eye.y,
  ĥz·10 − eye.z, l̂.z); q3 = 0.
- **The 10** is DAT_00564984, stored only by the reset `FUN_00489798` line 550211 (0x41200000) — one store, one load in
  the ELF (verifier); BF2 … 10.0 in every state read.
- q4 VU[148] = `SpecularColor`/255 (551604–551608 from DAT_0069c358..35b; key 0x1e4aaf7 → 550359; BF2 0x76c138, 707709
  → 709047). q5 VU[149] = `SpeckleAmbientColor`/255 + 65536 (0xba5f7683 → DAT_0069c368, 550399); q6 VU[150] =
  `SpeckleSpecularColor`/255 (0xa6b274b0 → DAT_0069c360, 550395). Reset defaults (`FUN_00489798` 550214–550226):
  Specular (192,160,192,192), Min (64,64,100,64), Max (100,100,128,255) — every water map authors its own.
- RAM, BF1 YAV1 state: q4 = 0.75294 ×4 (Specular 192), q0 = (1, −0, 1, 0.866), light vector L = (−0.557, −0.766,
  0.321); the same 23 qwords sit in `vu1Memory.bin` 144..166 at save time.

### 1.5 The colour LUT and the vertex index (claims 5–7, SUPPORTED)
- `FUN_0048d068` (header 551904): entry i ≤ 5 = Border + (Min − Border)·i/5; **i > 5 = Min + (Max − Min)·(i − 5)/5, so
  entry 10 = Max and entry 15 = 2·Max − Min**; stored value/255 + 65536. … never stored). BF2
  `FUN_00577858` identical, DAT_0060c1f4 = 5. RAM (BF1 YAV1, Border 160 / Min 32 / Max 128), GS R column:
  80 67 54 41 28 16 25 35 44 54 64 73 83 93 102 112.
- Each template vertex's word is `phase << 8 | (5 + r % 11)` (`FUN_00489f50` 550499–550500); the remainder is DIVU
  (ELF 0x48a2d4 = 0x0055001b), so i ∈ 5..15. **[D] The mean vertex colour ≈ MaxDiffuse** — near-uniform over 5..15 on
  the near templates (9×9 on YAV1, 17×17 on KAS2); the last row and column copy the first, and the FAR templates are
  tiny (YAV1's 2×2 gives all four vertices index 13; KAS2's 5×5 shows only 8 of the 11 values) (verifier).
- `FUN_00398928` (header 386049) sends that stream with STROW 0x47800000 ×4, STMOD 1 (offset) and a V4-8 unsigned UNPACK
  (`| 0x6e000000`, flags 0xc000) → x = 0x478000|index, y = 0x478000|phase; `ILWR.x/.y` read the low 16 bits.

### 1.6 The alpha column (claim 8, SUPPORTED)
`FUN_0048cec8` (header 551852): entry j = (Min.a + (Max.a − Min.a)·tri(frac(t·DAT_0056498c + j/15)))/255 + 65536,
tri = the 0..1 triangle wave; the vertex's phase nibble picks j. [D] Its mean is (Min.a + Max.a)/2 — the port's value.
It also writes a complementary ramp into the second table, which only the dead pass of §1.8 would read.

### 1.7 Routine A — the speckle (BF1 0x74e–0x76b, BF2 0x758–0x775; claim 9, SUPPORTED)
vf7 = (p + VU[145].xyz) × VU[144].xyz; F = max(0, (VU[144].w, VU[145].w, VU[146].w)·vf7/|vf7|)^32 (five `MUL.x`
squarings 0x75f–0x763); colour = VU[149] + VU[150]·F, clamped. Every water map authors `LightAzimAndElev` (1, 0) (the
12 manifests; RAM on every state read), so vf7 is horizontal and l̂ = −ĥ: **a cos³² lobe around the sun's azimuth,
offset by `SpeckleCoordShift` along ĥ** — the glitter path. Its alpha (the add weight, TCC cleared on the near speckle,
`re-water-alpha.md` §7.1) is the same sum's w: SpeckleAmbient.a + SpeckleSpecular.a·F.

### 1.8 A second main-layer pass that never runs (claim 10, SUPPORTED)
`FUN_0048b9c0` 551365 `if (DAT_00522a5c != 0)` would draw the tiles again with the scroll reversed and a second table
(551372 *(decompiler detail omitted)*); BF2 `FUN_00575fd8` 708792 `DAT_0060c1dc`. Both are 0 in the ELFs;
the only access to each is one load (BF1 0x48bd9c in `FUN_0048b9c0`, BF2 0x5764c4 in `FUN_00575fd8`) —
`lead_tmp/re/wse/addrscan.py`, the pointer candidates read by hand (byte-table `lbu` reads at 0x522a20 / 0x60c1a0, the
0x5229c0 store in `FUN_00474d98`, BF2 0x60c1c8 = the console variable `render.ForceFullDetail`); no data word in either
ELF equals either address; 0 in all 113 states (verifier).

### 1.9 The ocean's colour (claim 11, SUPPORTED) — answers HANDOFF What next 2(d)
BF1 `FUN_0048a6c8` (header 550630) 550693–550695 reads DAT_0069c420..422 and 550727–550728 packs `(x + 1) >> 1` per
channel with the foam byte as alpha. Those bytes are the engine's global WHITE: `FUN_00492598` (header 555042) stores
0xff to 0x69c420..423 (555048–555049, 555070–555071), the only store (verifier); 255 in all 55 BF1 states. So the ocean
base draws at GS 128 = unity — texel × 1.0, unlit — and the ocean path (`FUN_0048b280`) never loads the overlay.
**The port tints its ocean with the flat-water LUT midpoint** (`water_look`) and lights it; F18's PS2 sea patch
(15,29,25) is the `kam1_water_2` mean (20,33,28) at unity [D]. BF2's ocean (`FUN_00575670`) is not walked here: it
sends the same colour command as below [H].

### 1.10 BF2's extra colour command [H]
Before its main layer and its ocean, BF2 sends `FUN_00497cf0` (command 0x43a: up to four colours, ×1/128, into VU[11],
[20], [29], [38]) with the float colour at 0x76c258 × `DAT_007c5754`. In RAM 0x76c258 = (1,1,1,1) on all 8 BF2 states
read, and `DAT_007c5754` = 0.5 on six maps, 1.0 on BF2 YAV1 and DAG1 (set by `FUN_0057fd40` from `FUN_00456098`:
1.0 when its second argument is 0, else 0.5). Routine B reads none of VU[11..38] (§1.2). [H] that the command is inert
for the water; that it scales the LIGHT colour of lit BF2 draws is a lead for queue LITE (BF2 renders brighter in the port).

## 2. The PS2's draw taken apart in PCSX2 [V measured][O]
Save-state copies with the `Effect("Water")` colour bytes patched in EE RAM (BF1 0x69c340.., BF2 0x76c120..; the EE
rebuilds the block, LUT and alpha table from them every frame), booted in the isolated rig and quick-saved ~15 s later
(`ps2exp/mkstates.py`, `run_ps2exp.py`; every variant stood at the same pose to the mm). Box means over water-only
boxes (`compare.py`; `ps2exp/frames/`, `sheets/wcol_<map>.png`), averaged per map:

| map (boxes) | PS2 | Specular 0 | speckle 0 | bed (alpha 0) | the port's model on the PS2 | port today | port bed | port + the PS2 terms |
|---|---|---|---|---|---|---|---|---|
| BF1 KAS2 dock one (4) | (94,111,99) | (89,90,76) | (90,108,96) | (107,96,77) | (108,105,90) | (49,51,41) | (41,43,24) | (50,73,66) |
| BF1 YAV1 bazaar (3) | (69,88,72) | (70,86,65) | (71,88,74) | (95,113,81) | (96,110,89) | (75,77,65) | (83,85,68) | (66,75,66) |
| BF1 NAB2 canal (1) | (56,55,48) | (50,42,34) | (55,54,47) | (51,38,30) | (59,49,42) | (106,84,51) | (113,88,49) | (102,89,63) |
| BF2 FEL1 lake (3) | (35,54,41) | (35,45,30) | (33,52,39) | (43,45,21) | (42,49,32) | (85,90,52) | (93,99,49) | (76,84,47) |

"The port's model on the PS2" = Min = Max = their midpoint, `SpecularColor` 0, the speckle flat at `SpeckleSpecular`:
the PS2 then draws the port's grey water with a white mottle (`sheets/wcol_bf1_kas2.png`). Reload control: the YAV1
spawn state reloaded unpatched stands 25 cm off the saved eye (the droid shuffles, the tutorial banner fades); every
variant shares that reload, and its box means agree with the saved frame within 3–7 levels.

## 3. The port's draw taken apart at the same poses [V measured]
A scratch instrument (`lead_tmp/re/wcol/instrument/apply.py`, applied only in a detached scratch worktree, never on
main): env knobs add routine B's term (with the PS2's own light vector from the state's RAM), drop the lighting, set the
tint to MaxDiffuse, and give the speckle routine A's lobe; `run_variants.py` renders every variant at the PCSX2 poses
(`renders/`). Instrument check: the no-knob render equals the library's committed port frame byte for byte (max diff 0)
at every pose. Per map (the table above, "port + the PS2 terms"): the water's teal lift appears where the PS2 shows one
(KAS2 G/B +22/+25 over the port today); the port's speckle, +9..+18 today, falls to 0..+2 under the lobe (the sun sits
behind or beside these views). MaxDiffuse alone moves +0.5..+3; unlit alone < 1.

## 4. Side finding — the port's sun direction [V disc/RAM/code][O], consumer [H]
BF1 `FUN_004931e0` (header 555571, via `FUN_0049b8b8`) sets DAT_0069c590..598 = (−sin a0·cos a1, cos a0, −sin a0·sin a1)
from `sky_ SunInfo.Angle` (a0, a1) (555585–555589; `FUN_00434218` = sinf, `FUN_00434028` = cosf). The disc's angles
(140, −30) YAV1, (145, 280) KAS2, (145, −30) NAB2, (180, 0) KAS1 give exactly the RAM vectors (−0.5567,−0.766,0.3214),
(−0.0996,−0.8192,0.5648), (−0.4967,−0.8192,0.2868), (0,−1,0) (`lead_tmp/re/wcol/texrgb/src/bin/sunangle.rs`); the port
(`mungefs::sky::SunInfo::direction_to_sun`, negated in `bake::lighting::lighting_from`) bakes (−L.z, L.y, L.x) — on all
14 BF1 maps with states (verifier, ≤ 0.51°). BF2: the port's `lght`-quaternion sun equals (L.x, L.y, −L.z) of the
PS2's `*(DAT_0060c6e0)+0x50` on all 16 maps. `re-sky-lighting.md` §4 derived the port's two conventions from their
agreement with EACH OTHER on BF2 GEO1, not from the ELF. PCSX2 frame check: BF1 KAS2 `cp5_cp10_dock_one_v3_up` (yaw
339.3°) shows the sun glare top-LEFT, where the PS2's L puts the sun (yaw ≈ 350°, 55° up); the port's would be at yaw ≈
260°, out of frame. Other writers of 0x69c590: an init and a reset (0,−1,0) and a temporary swap that is restored
(verifier). **[H]:** that BF1's and BF2's DIFFUSE lighting of models and terrain uses this same vector — the next RE
(queue SUND) must read the light setup (`FUN_004b8e78`) and the VU lighting routines before the bake changes.
**Settled by `analysis/re-sun-direction.md` (SUND, 2026-09-26, [O]):**
- Models ARE lit with this vector, brought into model space on VU1. BF1's term is ((1 + n·l)/2)⁴; BF2's is max(0, n·l).
- The terrain is unlit (TERL).
- `FUN_004b8e78` is the side-select preview's light-rig swap.

## 5. What the port owes (queue WCOLB, and SUND)
1. The flat water's main layer: unlit; colour (GS terms) = the LUT mean (MaxDiffuse; a per-vertex noise over entries
   5..15 is the faithful form) + `SpecularColor`·F; alpha = the table mean + `SpecularColor.a`·F; F from the fragment's
   view vector to eye − 10·ĥ; both clamped at 255/128; As = texel.a × alpha. PROVISIONAL [O]: the per-fragment F (the
   PS2 interpolates per vertex on a 4 m grid), the mean in place of the noise.
2. The speckle: colour = `SpeckleAmbient` + `SpeckleSpecular`·lobe³², weight = the same sum's alpha, lobe per §1.7.
3. The ocean base (both KAM1s): unity, unlit.
4. ĥ comes from the PS2's light vector — correct once SUND lands; until then derive it from the manifest's `sun_dir`
   per §4 (PROVISIONAL).
5. What it cannot fix: the bed (KTEX: KAS2's channel is sand; LITE/SUND: NAB2's and FEL1's beds are 2–2.5× too bright),
   BF1 KAS1's mirror (REFL), BF2 KAS2's far haze (F7/F8, SUNG).

## 6. What stays [H]
The stream at VU[2].w is world positions [D]; BF2's colour command is inert for the water (§1.10); BF2's ocean colour
path (§1.9); the PS2 blends in gamma space and the port in linear — at YAV1 the port with the PS2 terms came within
2–4 levels in R and B, so not a first-order term here [D]; whether any flat water map shows Av above 0x80 at play
distances (then As > 1 overshoots on the GS, the port clamps).

## 7. Verification (`OPUS_LOOP_PROMPT.md` §5)
Self-checks: (1) every decomp citation's enclosing function printed (`lead_tmp/re/wse/enclosing.py`, `funcmap.py`);
(2) the absence claims searched: the pass-2 flags by `addrscan.py` + a data-word scan of both ELFs, routine B's reads by
reading all 25 instructions, the port's view-angle term by reading `fs_water`/`shade_unfogged`; (3) BF2 opened for every
twin claim (overlay words diffed, flags, keys, LUT and alpha builders, dispatch table); (4) constants read as ELF words
(the chains, DIVU, DAT_00522a74 / 0060c1f4, the 10.0 store); (5) flag defaults: the pass-2 flags 0 with no writer, the
LUT-builder gates DAT_00522a6c/70 (BF2 0060c1ec/f0) 1 with no writer and (1,1) in every state, the shift 10.0 at the
reset and in RAM. **The verifier** (one Opus agent, claim list only, `verify/VERDICTS.md`): 11 SUPPORTED, 2 PARTLY, 0
NOT SUPPORTED. Corrections folded in: routine B does use the light direction (the 10 m shift, §1.2); the overlay gate
is the draw record's word, not VU[7].x (§1.3); "uniform" holds for the near templates only (§1.5); l̂ defined (§1.4).
Unchecked by it: the virtual callees that receive &0x69c420 (the two direct ones only read it).

## 8. Native template and Kashyyyk reference-state follow-up (2026-11-03)

[V] The maintainer's paused BF2 KAS2 slot 10 state was read without modifying
it. EE memory identifies near/far packets at 0xe8daa0 / 0xe90c70, 17 by 17
and 5 by 5 vertices. V4-16 positions paired with V4-8 colour/phase indices
agree at every one of the 289 near and 25 far vertices with the following
independently reconstructed sequence. Each template starts at 0x97331547.
The LCG step is s = 1664525*s + 1013904223 modulo 2^32. Four successive
states a,b,c,d produce colour index 5 + ((a >> 16) | (b & 0xffff0000)) % 11
and phase (c >> 16) & 15; d starts the next vertex. X is the outer loop,
Z the inner loop. Opposite edges copy the first edge without advancing RNG.
This replaces the port's provisional xorshift realization. Far colours can
be reconstructed from the initial unwrapped near-template sequence, rather
than by decimating its spatial samples. LODDecimation clamps to unscaled
PatchDivisions: Kashyyyk's authored 8 clamps to 4, producing 32 m far cells.

[V] In that same reference, ordinary fog is off: 0x746050/54 and
0x7c575c/60/64 are zero. The retained fog rows 0x7bb800 nevertheless contain
1/260 and -90/260. The far-water pass does not test the ordinary fog flag;
its authored colour at 0x7bb7f0 is RGB (104,106,108). This directly confirms
the retained-matrix possibility left open in re-fog.md section 2.1. The range
90..350 is inherited state, not Kashyyk's authored FogRange(0,0). Its prior
mission provenance and fresh-boot appearance have not been traced. The port
uses this measured reference range as a stateless visual correction on BF2
KAS2 only, retaining the colour from the player's own sky data and leaving
world fog off. It does not emulate arbitrary prior-map fog history.

Local receipts and captures are under engine/artifacts/water_fidelity/.
The separate zero-water diagnostic state was prepared but not loaded: Windows
menu targeting failed, and the emulator was left paused at its original view.
No original-game zero-water bed comparison is claimed.

## 9. Flat-water HDR amplification (2026-11-03)

[V] A new GS dump from the running BF2 Kashyyyk reference confirms the
post-composite ALPHA register 0xff00000029, with FIX 255: the framebuffer is
multiplied by 255/128 before adding glow (re-glow.md section 2.2). The dump
contains 208 vertices using that state. The water's overlay routines A/B
ignore BF2's HDR scene-colour command (section 1.10), so their RGB is not
halved before this composite. The resulting flat-water surface and speckle
contributions therefore receive 255/128 display-colour gain. Ordinary world
geometry already cancels its half-scale scene colour against this composite.

[V] Main-water TEX0 is 0x2007a00621313c00 (256-square PSMT8, palette at
0x3d00), ALPHA 0x4144. Its uploaded palette holds source RGB without doubling
and alpha 79, half the source 158. Near speckle TEX0 is 0x198007000,
64-square PSMCT32 with TCC cleared; far speckle uses 0x2007a3859930bd04,
64-square PSMT8, palette at 0x3d1c. Their ALPHA is 0x4148. This supports
amplifying colour without increasing surface or speckle opacity. The water
fog vertices have RGB (26,26,27), consistent with their authored fog colour
(104,106,108) after GS modulation and HDR amplification. The renderer's
already display-scaled fog colour must not receive a second gain.

The port now applies 255/128 in display space to base water and speckle RGB
on BF2 HDR maps using the flat-water path, before its separate water-fog
composite. BF1, non-HDR maps and the separately fitted FFT ocean retain unity.
This restores a missing measured amplitude rather than introducing a foam
contrast curve. No asset rebake is needed for this shader correction.

The dump, screenshot, parser and comparison captures remain local under
engine/artifacts/water_fidelity/contrast/. The reference was briefly resumed
to record a GS frame and paused again; save states were not overwritten.
GPU referees cover RGB gain with unchanged transparency, bed colour and fog.
Same-camera port captures compare the preceding build and corrected build;
they are not pixel-registered PS2 comparisons or a full map visual sweep.

## 10. Far water sampling and precision (2026-11-03)

[V] The same GS frame from section 9 has 1,615 packed vertices in each
far-water draw (main, speckle and fog). The main and speckle records write
PSMCT16S (`FRAME` 0x000000000a080180) with DTHE 1 and DIMX
0x2031203131203120. The near records write CT32 at 0x0000000000080080
with DTHE 0. The four DIMX rows in screen order are (0,2,1,3), (0,2,1,3),
(1,3,0,2), (1,3,0,2). The far target stores its RGB channels in five bits.
[PCSX2's GS format definitions](https://github.com/PCSX2/pcsx2/blob/master/pcsx2/GS/GSLocalMemory.cpp)
record the corresponding 0xf8 channel masks.

[V] Kashyyyk's base texture carries one source mip. Its GS TEX1 is
0xffffff8d00000120 (MXL 0, linear filter and K = -115/16). Far speckle
uses 0xffffff8d00000128 (MXL 2), while the near crossfade target uses
0x120 (MXL 0). The port previously sampled the completed baked mip chain
with zero bias and filtered sRGB textures in linear light before converting
back to display bytes. The flat-water uploads now filter the stored bytes
with the measured GS bias. The renderer retains its baked mip chains and
uses GPU derivatives for LOD selection.

[V] The far speckle uses TCC=1 and the current flipbook frame. The near
speckle uses TCC=0 and two cross-faded frames. For Kashyyyk, all 25 source
frames' level-0 texels have alpha 255. After accounting for the GS palette
layout, all 173 used colours of captured frame 0 match the disc palette
and have GS alpha 128. The zeros in the first palette check were unused
entries. The discarded bake diagnosis had attributed those zeros to the
visible texture. The renderer now switches the far layer to the current
frame's texture alpha while preserving the near crossfade. It mixes those
contributions over the authored near/far depth band.

[D] For an additive write onto an RGB5 destination, the stored destination
is already a multiple of eight before the addition. The rounding can
therefore be applied directly to the added speckle contribution before fog
attenuation and HDR display gain. A weak highlight adds five GS levels in
the GPU referee and rounds to zero or eight under the captured DIMX matrix.
The far diffuse grid also interpolates its colours with screen-space
weights, using the corresponding projected corner depths. The base water
and fog use continuous destination blends. The renderer's dither is aligned
to its target pixels.

[V] As cross-checks, all 1,615 matched main/fog vertices recover the
90..350 m fog coordinates (S/Q fitted to 1/Q). At 1,546 positive-depth
integer-UV main vertices, the captured RGB and alpha agree with the
reconstructed diffuse table to 2.13 authored levels at p99. The captured
speckle alpha agrees with the baked light azimuth to 0.67 GS levels RMS over
506 on-screen vertices. These tests distinguish the sampling and precision
change from edits to the water palette, fog range or light direction.
