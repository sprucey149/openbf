# OPUS RE (loop lane 2, 2026-09-25) - [O] UNAUDITED: no Fable pass
# RE: the terrain's PS2 lighting — both games draw the heightfield UNLIT, texture × COLR at a per-game scale (queue TERL)

**The question** (card TERL; `analysis/re-model-lighting.md` §5, which left the form and scale [H]): the port lights the
terrain as LUMI × (ambient + sun·N·L). Both Hoths clip to white (F27, HOLD 11(a)) and BF2's darker maps come out too
bright. What does the PS2 draw? The acceptance asks for:
- the terrain's VU program, which per-vertex colour it takes, and its scale (does the HDR 0.5 reach it?);
- the ~7 % residual at BF2 HOT1;
- what the light set's +0x94 light is;
- BF1's terrain path, settled by a BF1 patched-state pair.

Method: the loop's RE discipline — the hops below, the five self-checks (`engine/artifacts/lead_tmp/re/terl/checks.md`,
verbatim), then one Opus verifier given the claim list only (`…/terl/claims.md` → `…/terl/verify/VERDICTS.md`). Three
measurements back it:
- the PS2 taken apart in PCSX2 (patched save states, §8);
- a disc census of COLR, the layer masks and the lights (§7);
- the port's terrain drawn by the PS2 rule at the library poses (a scratch instrument, §9).

Scratch, tools, frames and sheets: `engine/artifacts/lead_tmp/re/terl/`. Conventions:
- B2 = `analysis/SWBFRNT2.ELF-decomp.c`, B1 = `analysis/SWBFRONT.ELF-decomp.c`.
- VU1 = the main program in `.vutext`, decoded with `scripts/tools/vu1dis.py` (dumps `terl/vu1_bf{1,2}_full.txt`).
- "GS units": 0x80 = 1.0. "c5" = one 5-bit COLR channel.

## 0. The answer

| | BF1 | BF2 |
|---|---|---|
| **base pass** | UNLIT (VU1 unlit path), per layer | UNLIT (VU1 unlit path), per layer |
| vertex colour to the GS | **c5 × 8** per channel (≤ 248; the copy path) | **c5 × 4 × HDR** (≤ 124 × HDR; multiply path at HDR 0.5, copy at 1.0) |
| pixel | tex × c5 / 16, clamped at 255 | tex × c5 / 32 (HDR on: ×0.5, then the composite's ×255/128) |
| ambient / sun / Light2 | **none** (measured: §8) | **none** (measured: §8) |
| LUMI | not read by any terrain draw; a point lookup (soldiers, foliage) | same |
| extra passes | none | up to 4 non-`Static` OMNI lights + one PROJECTOR light, additive (§5) |
| detail pass | DTLX texture, frame × 2·alpha on the 3×3 patches round the eye | same |
| layers | up to 16 LTEX per patch, 4-bit per-vertex weights, alpha blend | same |

- **The HDR 0.5 reaches the terrain and is neutral.** The pass colour is 1.0 × HDR × 128 (§1). The HDR composite that
  doubles the frame back runs only while the scale is 0.5 (§1.4).
- **The per-map "neutral" rule the port uses (LUMI 128 vs 255) is the per-GAME scale.** BF1's lightmaps are authored
  round c5 = 16 (× 8 = 128 = GS 1.0), BF2's round c5 = 31 (× 4 = 124). The exceptions follow the game, not the map:
  - BF2 FEL1 / NAB2 / YAV1 / POL1 are drawn at 0.22–0.47 of the texture;
  - BF1 BES2's c5 = 31 third at 1.94.
- **The HOT1 residual.** COLR 31 gives 124/128 × 255/256 = **0.965**, so the lightmap-only 229.5 becomes 221.5 (the
  instrument: 223.2 / 221.2 at the LITE boxes, the PS2 210.4 / 206.5).
  - **[H]** The remaining 4–6 % is the LAYER blend the port does not draw. The spawn's patches are masks 0x5 / 0x7
    (hoth_main_2 mean 177, hoth_main_4 214, hoth_main_1 225 on the disc).
  - **2026-09-28 (`re-terrain-layers.md` §7, card TALPH): refuted on BF2, partly true on BF1.**
    - BF2: every vertex under the four LITE boxes paints hoth_main_2 / _4 at weight 0 or 0xf, so the ground there is
      hoth_main_1 alone. The cause is open.
    - BF1: the spawn boxes' visible layer is hoth_main_4 (mean 214). The layers close 28–45 % of BF1's gap.
  - BF1 HOT1 shows the same gap (PS2 / rule 0.88–0.91, same masks).
- **The +0x94 light is a PROJECTOR** (§5). It is the strongest non-`Static` spot light near the patch that authors a
  `Texture`, else the first directional light that does. It is drawn with its `PS2BlendMode` and is off in
  split-screen. No light on a BF2 terrain map authors a `Texture` (§7), so it never fires on the shipped maps.

## 1. BF2's base pass (EE) [V][O]

1. **The draw.** FUN_00571bd0 (B2 706418) runs per visible patch:
   - builds a light set `…
   - FUN_00599040 has no other caller but FUN_005719f0 (706394), itself a terrain pass.
2. **Per layer** (FUN_0056f208, 16 chains per patch):
   - The "lit" branch is taken only when the layer is the patch's top painted layer (+0x48) AND the set has a +0x94 light
     or omni lights (705214–705215).
   - Otherwise, while DAT_0060c1b4 ≠ 0 (705396; ELF byte 1, no store anywhere), the "plain" branch runs:
     - descriptor 0x76c080 for pass 0 and 0x63c690 for passes 1–3 (705409);
     - pass colour = DAT_0076c258..264 × FUN_0057fd50() × 128, packed R | G<<8 | B<<16 | 0x80000000 (705411–705424;
       only R, G, B are scaled — alpha is 0x80, verifier A2);
     - the chain kick (705431–705435).
   - FUN_0056f208 reads the set only at +0x94, +0xa1 and +0x78 + 4i (every *(decompiler detail omitted)* in the function; verifier A3: the
     only fp-relative reads).
3. **The constants.**
   - DAT_0076c258..264 = 1.0: FUN_0057ca38 712445, its only store.
   - FUN_0057fd50 returns DAT_007c5754, the HDR scale (715188). Its writers are FUN_0057fd40 (715178) — called only by
     FUN_00456098 with 0.5 / 1.0 (516764 / 516761) — and the reset FUN_00580c80 (1.0).
   - So the pass colour is **(HDR, HDR, HDR, 1)**.
4. **HDR is neutral.** FUN_00456098 calls FUN_00451678, which registers the HDR render pass (vtable +0xc, 513124) when
   `Enable` ≠ 0 and unregisters it (+0x10, 513128) otherwise, then sets 0.5 / 1.0.
   - The composite's ×255/128 (`re-model-lighting.md` §1) exists only while the scale is 0.5.
   - RAM: BF2 FEL1 0.5, **BF2 YAV1 1.0** (its HDR is off; §8).
5. **The descriptors.**
   - 0x76c080 = {0x00100044, 0, 0x50000, u16 0x58}: FUN_00573010 707232; ELF 0x573054 `lui t2,0x10; ori t2,t2,0x44`,
     `sd` at 0x76c080.
   - The low byte 0x44 is the GS ALPHA blend (Cs−Cd)·As+Cd. Word 0 >> 8 = **0x1000** is the pass flags (§2).
   - 0x63c690 = {0x300, 0, 0x30000, 0} (ELF bytes): flags 3, a disabled pass.
6. **The vertex colour** (FUN_0056c9d8, the patch builder; 703869–703871 and five copies; ELF 0x56cf6c–0x56cf8c
   `sll 2 / andi 0x7c`, `sll 5 / andi 0x7c00`, `sll 8 / and 0x7c0000`):
   - R = (c & 0x1f) << 2, G byte = bits 5–9 << 2, B byte = bits 10–14 << 2;
   - A = DAT_0060c1a0[the vertex's 4-bit layer weight] (§6).
   - c is the u16 FUN_00572d38 decompresses from the patch entry's +4 word (FUN_0056e898 *(decompiler detail omitted)*, 704644).
     The PCHS parser FUN_0056bbf0 stores `COLR`'s per-patch pointers there (703351 / 703364) and `ALPH`'s at +0
     (703310 / 703323).
   - A cutter-replaced patch (entry +0x10 set) takes FUN_0056dcd8, same formula, its own `PTCH` COLR (verifier A6).
7. **The batch** (FUN_00494af8 via 704230):
   - header → VU1 handler 0x47c (553796);
   - STMOD 1 (553800), the colour stream UNPACK V4-8 unsigned (553901) onto STROW 0x47800000 = 65536.0 (553897), so each
     channel sits in VU memory as 65536 + c/128;
   - the batch ends with commands 0x606 and 0x3a4 (553917–553918).
   - The GIF tag the VU kicks lists ST, RGBAQ, XYZ2 (verifier C4): the stream's x/y/z/w are the vertex's R/G/B/A.

## 2. The VU1 path, both games [V][O]

- **Descriptor.** 0x406 (BF2) / 0x404 (BF1) stores each pass descriptor at VU[8+9k] / VU[8+8k]. It writes word0 >> 8 —
  MTIR of ITOF0(word0) + 2^31, exact for 0x100044 → 0x1000 and 0x300 → 3 — to VU[14+9k].w / VU[14+8k].w, the pass flags.
- **Colour.** 0x43a / 0x438 stores the pass colour's four bytes × 1/128 at VU[11+9k] / VU[11+8k] (0x445–0x446 /
  0x443–0x444).
- **The pass loop.** 0x606 / 0x5fc runs the per-pass routine 0x162 / 0x15d (BAL → 0x962 / 0x95d) four times, vi15 = the
  block base.
  - Precondition (verifier B3): OR of the four flags & 0x108 == 0. It holds for every terrain descriptor (0x1000, 3, 1, 0).
- **The per-pass routine.** flags & 3 == 3 skips the pass (0x166 / 0x161). flags & 0x40 == 0 takes the **unlit path**
  0x2a3 / 0x2a7 (0x174–0x178 / 0x16a–0x16e). The terrain's 0x1000 has no 0x40.
- **The unlit path.** With (flags & 0x1000), the colour stream present (VU[7].z & 0x1000) and not (flags & 0x20), it
  goes to 0x2da / 0x2de (the only branch in: 0x2b1 / 0x2b5).
  - If the pass colour is exactly (1,1,1,1) (SUBw + FMAND zero flags) the stream is **copied** (0x304 / 0x308).
  - Otherwise colour = (COL − vf31.x)·vf1 + vf31.x, clamped by MINIi at 0x478000ff.
  - vf31.x is `MAXi` in the same pair as `LOI 0x478000ff`, so it reads the PREVIOUS I, 65536 (LOI at 0x2ae / 0x2b2). An
    I-bit pair's upper op sees the old I: PCSX2 `VU1microInterp.cpp` writes REG_I after `_vu1ExecUpper`; microVU runs
    `doUpperOp` before `doIbit`. So the path is a **multiply**, c × k, clamped at 255.
  - **[H] on hardware:** only PCSX2 and design intent were checked (verifier: weakest link). The measurements agree:
    - the other reading, 255 − (255 − c)/2 at HDR 0.5, would paint BF2 HOT1's snow at 1.48 × the texture, doubled by
      the composite;
    - the PS2 shows 205–214 with 0.2 % clipping (`re-model-lighting.md` §7), and FEL1's ground doubles when HDR goes to
      1.0 (§8).

## 3. BF1's base pass [V][O]

- **The draw.** FUN_00487ad8 (549070) calls FUN_004857b8 per visible patch (549275).
  - That only sets the layer texture (FUN_0049e1e0), a matrix (FUN_0039c3d0), the flags (FUN_0039bc48) and kicks the
    chain: no light set, no colour, no descriptor (548017–548081).
  - **BF1 has no lit branch on the terrain.**
- **Baked into the chain.** The descriptor and colour are put in when a patch is built, by FUN_004852e0:
  - `…
  - `FUN_0039c4f8(ctx, 0x80808080, 0,0,0)` (547929, command 0x438; ELF 0x48554c `lui a1,0x8080 … ori a1,a1,0x8080`).
  - 0x69c2e0 = {0x100044, 0, 0x50000, 0x58}: FUN_00488bb8 549875, its only store; ELF 0x488bf4.
  - 0x54a5a0 = {0x300, 0, 0x30000, 0}.
  - So BF1's pass colour is **(1,1,1,1)**, the copy path.
- **The vertex colour.** FUN_004828b8 546779 and copies; ELF 0x482e68–0x482e8c `sll 3 / andi 0xf8`, `sll 6 / andi 0xf800`,
  `sll 9 / and 0xf80000`:
  - **R = (c & 0x1f) << 3**, G byte = bits 5–9 << 3, B byte = bits 10–14 << 3;
  - A = DAT_00522a20[nibble], the same 16 bytes as BF2's table;
  - c comes from COLR (entry +4, FUN_00481608 546061–546075, decompressed by FUN_00488910).

## 4. COLR, not LUMI [V][O]

- The LUMI buffer (BF2 DAT_006615c4, BF1 DAT_005648cc) has exactly two accesses each:
  - the loader store;
  - a bilinear point lookup returning LUMI/255 (FUN_0055f1d0 695200 / FUN_00473d28 537203, called by FUN_0055f068 /
    FUN_00473bc0).
- That feeds entity and foliage brightness, never a terrain draw (checks (c); verifier D1, ELF-level including BF1's
  gp-relative accesses).
- **The terrain's colour is COLR, used unconditionally.** The port's hue gate (`colr_carries_hue`) and LUMI intensity are
  ours.

## 5. The light set's +0x94 and BF2's lit branch [V][O]

**The set.** FUN_00599040 returns 0xa8 bytes (checks; verifier E1):
- **Directional lights** at +0x70 / +0x74, at most 2, count +0xa0: the global Light1 / Light2, then type-1 lights.
- **Omni lights** (type 2) at +0x78.., at most 4, count +0xa1. In a prelit set the walk stops at the first `Static` one.
- **One spot** (type 3; prelit: non-`Static`) at +0x88, count +0xa2.
- **+0x94** = that spot if its +0xc ≠ 0 (730575), else the first directional light with +0xc ≠ 0 (730327).
  - It is zeroed when FUN_00563f78() > 1 (split-screen) or DAT_0060c6d8 == 0 (730664–730665). DAT_0060c6d8's only
    writer is *(decompiler detail omitted)* (FUN_00597388); ELF 1.
  - Edge (verifier): with no directional and no omni light gathered, a default set is returned. That cannot happen while
    Light1 exists.

**Light types** (word 2):
- FUN_00595968 = 1, directional (727765; it also builds the global lights);
- FUN_0059a550 = 2, omni (730769);
- FUN_0059bbc8 = 3, spot (731696).
- FUN_00597918 files them into DAT_007cd398 / 368 / 380 (729144 / 729120 / 729158).

**The light's fields.**
- +0xc is its **`Texture`**: FNV-1a("texture") = 0x3c6468f4, FUN_00596978 728665 → 728556.
- +0x10 is an address flag (5, or 0).
- +0x14 is its **`PS2BlendMode`**: 0x84e26526, 1 → ALPHA 0x89 (multiply), 2 → 0x44 (blend); 728601 → 728566.
- **So +0x94 is the patch's PROJECTOR light.**

**The lit branch** (705216–705393), on the top layer only:
- **Pass 1** is the projector: its texture, its blend, the matrix from FUN_0056ee08.
- **The next passes** are up to 4 omni lights, each drawn with:
  - colour = its RGB (+0x1c..+0x24) × HDR × max(0, 1 − dy²/range²), where dy = the light's height (+0x54) above the
    terrain at its (+0x50, +0x58) and range = +0x60 (705302–705335);
  - texture 0x36462699 and descriptor 0x76c0c0 = 0xff00000148 (ALPHA 0x48, **additive**, flags 1).
- In a prelit set these are the non-`Static` omnis. No BF2 terrain map authors one (§7), so they are **runtime** lights.
  - The library shows them: the blue pool round BF2 YAV1's command post, the brighter floor round NAB2's CPs (§9).
- BF1 has no such branch (§3).

## 6. The detail pass and the layers [V][O]

- **The detail pass.** With DAT_0060bed6 ≠ 0 (ELF 1, no store anywhere), FUN_00571bd0:
  - sets texture DAT_006615c0 (706678), the terrain's DTLX detail texture, and calls FUN_00570f68 (706690);
  - FUN_00570f68 draws **the 3×3 patches round the eye** (FUN_005700a8) with descriptor word 0x89 (706151: ALPHA Cd·As,
    flags 0 under the VU's chop rounding, verifier F1) and colour 0xff808080 (706155).
  - The vertex alpha is 0xff, so As ≈ 2·At: **the frame × 2 × detail alpha**, the same form as the port's `_dtl`
    modulate.
  - BF1: FUN_00486e30, same colour (548780). Its gate DAT_005229b0 is 1 in the ELF; its two stores at 0x472690 / 0x4726a8
    have no `jal` and no data-word reference anywhere in the ELF (reproduced).
  - FUN_005719f0 is gated by DAT_0060bed7 = 0: off.
- **The layers.** A patch draws up to 16 LTEX layers, each blended with ALPHA 0x44. A vertex's alpha is
  DAT_0060c1a0[its 4-bit weight] = {0, 9, 18, 27, 36, 45, 54, 64, 73, 82, 91, 100, 109, 118, 128, 0}.
  - A single-layer patch uses 0xee (weight 14 → 0x80, opaque).
  - Triangles whose three weights are all 0, or any is 0xf, are skipped (FUN_0056c9d8).
  - The weights come from entry +0 (`ALPH`), which FUN_00572c98 decompresses into 4-bit nibbles. That is queue TALPH's
    open format.
  - **Read 2026-09-28: `re-terrain-layers.md`** has the format, the draw order, the skip rule, the per-layer UVs and the
    cutter patches. Correction to the skip rule: BF2 exempts each patch's top layer; BF1 does not.

## 7. The disc [V measured]

Census: `terl/colr_census` (a scratch mungefs tool); output `terl/colr_census.txt`, `terl/light_census_bf2.txt`.

**COLR g5, the modal value:**
- BF1: HOT1 16 (99.2 %), RHN1 16, NAB1 15–18, KAS2 15 / 0 / 4, TAT2 19, BES1 / KAM1 19 (tinted), GEO1 12–15, TAT1 11–14,
  YAV1 12, END1 10, KAS1 9, NAB2 9, RHN2 9, YAV2 6, BES2 16 / 31 / 9.
- BF2: HOT1 / END1 / KAS2 / DAG1 / TAT2 31, GEO1 25, POL1 12–15, YAV1 12, FEL1 11 (22 on 9.6 %), NAB2 7 (tinted, B > R).

**The lights on BF2's ten terrain maps** (DAG1, END1, FEL1, GEO1, HOT1, KAS2, NAB2, POL1, TAT2, YAV1):
- **0 non-`Static` omni lights and no light with a `Texture`.**
- Projectors exist only on DEA1 and MUS1, which have no heightfield.

**DTLX detail textures and their disc alpha means:** hoth_detail 128.3, yav_detail_1 125.2, nab2_detail_1 128.0,
end_detail_2 125.3, fel1_det02 100.7 (→ ×0.79 near the eye). All ≈ neutral on average except FEL1's (`terl/texdump`).
The bake stores them opaque, so their alpha is lost in `assets_baked`.

**Layer masks round the spawn eyes:** multi-layer at almost every library spawn (CP sites). Single-layer (0x0001) at BF2
NAB2's three spawns and BF1 NAB2's rotunda.

## 8. The PS2 taken apart [V measured]

`terl/ps2exp/` (mkstates.py, run_ps2exp.py, means.py; frames and `sheet_*.png`). Box means are luma on the state's own
640×480 frame.

| state (box) | control | sun 0 | ambient 0 | both 0 | HDR 0.5 → 1.0 |
|---|---|---|---|---|---|
| **BF1 HOT1 spawn**, snow (430,300,640,420) | 197.9 | 197.9 | 197.9 | 197.9 | — |
| BF1 HOT1, soldier (250,280,380,450) | 155.9 | 113.9 | 64.6 | 24.1 | — |
| BF1 HOT1, scaffold (0,60,160,200) | 139.7 | 104.8 | 86.3 | 38.0 | — |
| **BF2 FEL1 lake** (sun NOT `Static`), ground (400,300,640,470) | 27.0 | 26.9 | — | — | **54.1** |
| BF2 FEL1 lake, soldier (285,250,370,400) | 44.3 | 32.0 | — | — | 91.8 |
| BF2 HOT1 spawn (sun `Static`; LITE §7) | 217.8 | 217.8 | 217.8 | — | — |

**Acceptance 4 is met.** BF1's terrain does not move when the sun and the ambient are zeroed; every model does. BF2's
does not move under a non-`Static` sun either, and it doubles with the HDR scale.

## 9. The port drawn by the PS2 rule [V measured]

**The instrument.** In the detached scratch worktree `terl-scratch`, never committed:
- `bake/src/terrain_glb.rs` TERL_PS2=k: COLR c5 × k / 128, linearised like `col0_linear`;
- `game/src/viewer/scene.rs` TERL_UNLIT: the terrain on `LightPath::Unlit`;
- `render/src/frame.rs` TERL_NOFOG.
- Runner: `terl/run_port.py`. Sheets: `terl/sheets/`. Box table: `terl/means.txt`. Masked table: `terl/masked.txt`.
- **Instrument check:** with no knobs, the render equals the library's `port/` frame (max diff 0) on BF1 / BF2 YAV1,
  FEL1, BF2 HOT1 v2 and BF1 KAS2 / NAB2. Elsewhere the difference is LITEB's model lighting, which landed after the library.

**The clean absolute check** is BF2 NAB2 `cp5_cp2_post4_spawn`: single-layer patches, away from the CP's light.

| box | PS2 | the PS2 rule | the port today | today's colour, unlit |
|---|---|---|---|---|
| (400,300,640,470) | (48,38,31) L39.8 | **(48,37,29) L38.9** | (50,42,42) L43.7 | (135,109,82) L112.5 |
| (180,440,260,480) | (41,32,26) L33.6 | **(41,31,24) L32.8** | (46,39,39) L40.4 | (126,102,77) L105.2 |

On the same map the PS2's floor is blue-shifted against its texture by the COLR ratio (B/R 1.3 vs COLR 1.29–1.4),
which confirms the RGB channel order.

**The per-game factor.** The other factor misses by 2× wherever measured:
- BF1 HOT1 at × 4 would be ~113 against the PS2's 197–199;
- BF2 HOT1 at × 8 would clip white against 205–213;
- BF2 NAB2 at × 8 would be 2× the PS2.

**Hoth** (LITE's boxes, luma):

| pose | PS2 | the rule | lightmap only today | today |
|---|---|---|---|---|
| BF2 spawn | 210.4 / 206.5 | 223.2 / 221.2 | 230.6 / 228.5 | 242.8 / 243.4 |
| BF2 v2 | 213.1 / 211.8 | 217.6 / 219.9 | 224.8 / 227.2 | 238.0 / 243.6 |
| BF1 spawn | 197.4 / 192.4 | 228.1 / 223.0 | 228.1 / 223.0 | 242.5 / 237.9 |

**The residuals have named causes** (masked, PS2 / rule):
- Hoth 0.88–0.96: layers, §0.
- Near CPs 1.2–2.1: the runtime omni pass, §5 — NAB2 v1 / v2, YAV1.
- FEL1 lake 0.48–0.62, BF1 YAV1 / END1: foliage, water, fel1_det02.

## 10. What the port owes

**Queue TERL (build).**
- **Bake.** The terrain's `COLOR_0` becomes COLR's GS vertex colour: c5 << 3 on BF1, c5 << 2 on BF2, per channel, as
  Unorm8.
  - This drops `lumi_neutral` and the `colr_carries_hue` gate for the colour.
  - The game is known per map (BF2 carries `lght`, or the models' BF2 material format); the build picks and pins it with
    a test over every terrain.
  - Cutter patches keep sampling the grid colour; their own `PTCH` COLR belongs to TALPH.
- **Render.** A terrain light path: texel × sRGB→linear(vertex × 255/128) × detail. There is no ambient, no sun and no
  Light2. The GS factor reaches 1.94 on BF1, so the shader decodes it (Unorm8 cannot hold > 1).
  - Fog stays the world's; that is FOGN's.
- **Moves.** Every map golden with a heightfield, plus `tat1_*`; `rebake=tat1,tat2,maps`. Sweep, arena and sim cannot
  move: the heightfield positions are unchanged.

**New queues.**
- **TDTL:** the DTLX detail pass (both games), frame × 2·alpha near the eye. The bake keeps the alpha. How far out a
  modern port draws it is the build's call.
- **TDYN:** BF2's additive omni passes on the terrain (CP lights, explosions — the runtime non-`Static` omnis) and the
  projector pass.

**For queue TALPH:** the weights are 4-bit per-vertex nibbles from `ALPH` (entry +0), mapped by the 16-byte table above
and blended with ALPHA 0x44 per layer.

## 11. What stays [H]

- The I-register timing on real hardware (§2): PCSX2 and the measurements only.
- The Hoth residual. On BF2 it is not the layers; on BF1 the layers are 28–45 % of it (`re-terrain-layers.md` §7,
  2026-09-28). The rest is open.
- The exact COLR→GS rounding in the port's sRGB approximation (the LITEB COL0 convention).
- FUN_00563f78's meaning (the viewport count is inferred).
- What 0x36462699 names (the omni pass's falloff texture).

## 12. Verification

**Self-checks** (`terl/checks.md`, verbatim):
- **fn:** all 54 cited lines are in the named functions.
- **absent:**
  - the patch draw's light-set reads;
  - BF1's per-pass uploads (every caller of FUN_0039c4f8 / FUN_0039c1a8);
  - LUMI (2 + 2 accesses);
  - the HDR setter's callers (2);
  - the light-set builder's callers (2);
  - the BF1 patch draw's calls;
  - every branch into the VU multiply / copy paths.
- **word:** the descriptors, colours, expansions, flag bytes, alpha tables and disabled descriptors (ELF).
- **writers:** DAT_007c5754 (2), DAT_0076c258 (1), 0x76c080 (1), 0x69c2e0 (1), DAT_0060c6d8 (1). DAT_0060bed6 / 7,
  DAT_0060c1b4 and BF1's DAT_005229b0 / b4 have no text writers; the ELF-level stores to 0x5229b0 are orphans.
- **Twin:** every BF2 function above has its BF1 twin opened and walked by callers (§3).

**The verifier** (one Opus agent, claim list only; `terl/verify/VERDICTS.md`, 177 tool uses): **22 SUPPORTED, 1 PARTLY,
0 NOT SUPPORTED.** Folded in:
- F1: BF1's gate has orphan ELF stores (§6); "flags 0" for 0x89 needs the VU's chop rounding.
- A2: only R, G, B are scaled.
- B3: the 0x108 precondition.
- E1: the default-set edge.
- B5: its weakest link is recorded in §2 and §11.
