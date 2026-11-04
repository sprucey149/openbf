# OPUS RE (loop lane 3, 2026-10-03) - [O] UNAUDITED
# RE: why some prelit floors take far less of a command post's light on the PS2 than in the port — the GS blends vertex colour in screen space behind the VU1 guard-band clipper; DEA1's killed post (card BPRL)

**The question** (card BPRL; `re-point-lights.md` §14 "Open"): UTA1's pen floor gains +0/+16/+38 under its own post in
the port and +0/+1/+3 on the PS2; DEA1's cp5_cp6 corridor wears a blue wash the PS2 lacks; COR1's cp2 floor brightens.
The card's lead was the model draw's flags (0x10, bit 31, 0x4000000). Method: the loop's RE discipline — the five
self-checks, one foreground Opus verifier on the claim list (§9) — plus PS2 RAM, a software model of the PS2's
rasterisation held against the PS2's own measured gain, and a port prototype scored at every stored PCSX2 pose.

Scratch, scripts, pictures: `engine/artifacts/lead_tmp/bprl/` (`pen_ram.py`, `setpool.py`, `vu_guard.py`,
`dea1_cp6.py`, `penterm.py`, `gsraster.py`, `uta_pred.py`, `pred.py`, `par_*/`, `verify/CLAIMS.md`,
`verify/VERDICTS.md`). B2 = `analysis/SWBFRNT2.ELF-decomp.c`; VU1 = the main program, read with
`scripts/tools/vu1dis.py`; VU[n] = quadword n of VU1 data memory. "Half-widths / half-heights" = clip x/w, y/w
with the screen edge at 1. Tags: [V] read or measured, [D] derived, [H] guess; load-bearing claims also [O].

## 0. The answer

**No draw skips the gathered set.** The pen is drawn lit, with the post's light in its set, by the same path as
TAN1's walls (§1). Three different things made the three pictures:

1. **UTA1's pen floor and COR1's cp2 floor: the GS blends a triangle's vertex colour linearly in SCREEN space**, not
   in perspective, and the VU1 clipper cuts a triangle only where it leaves the GS's 4096-unit drawing space (±7.99
   half-widths, ±9.13 half-heights) or the near/far planes (§3, §4). The stored poses stand the camera on the post, so
   the post's light sits under the eye. The pen floor there is one fan of 6–15 m triangles round a vertex 0.35 m
   under the light: that vertex takes the light whole, the rim none, and a screen-space blend leaves the lit vertex's
   colour in a band below the bottom of the screen. The port's GPU blends in perspective and shows 40–55 % of it at
   the bottom edge. A model of the PS2's clip and blend, drawn from the disc, gives the PS2's measured gain to 0.1
   level: **+1.2/+3.0 against the PS2's +1.2/+2.9; the perspective blend gives +20/+49** (§4).
2. **DEA1's cp5_cp6: the post is dead.** The conquest script calls `KillObject("CP6")`; the PS2's battle has six post
   lights and none at cp6. The port's battle drops CP6 too — only its renders with no battle (pose_render, the
   goldens, the walkabout) light every placed post (§6).
3. The same blend carries **COL0** and every lit colour, so big prelit floors near the camera are also brighter or
   darker in the port than on the PS2 without any light (UTA1's pen: COL0 0.55 in perspective, 0.80 on the PS2, §4).

A GPU-safe form of the PS2's blend — value = persp(a·wc) / persp(wc), wc = the vertex depth clamped to the guard
band — in a port prototype takes COR1's floor from 103/92/68 to 70/63/48 (PS2 70/65/51) and the all-maps parity from
10.672 to 10.639; plain `@interpolate(linear)` breaks (§5).

## 1. Which draws take the gathered set [V][O]

- **The gate.** `FUN_0057cf60` (RedModel vtable 0x64feb0 slot +0xc; ELF word 0x64febc, no `jal`/`j`) gathers and
  uploads only when `(flags & 0x80000010) == 0` (B2 712762). `DAT_0060c2b5 == 0` ORs 0x10 in (712758–712760); the
  byte is 1 in the ELF, has no store, and is 1 in all 178 BF2 states (verifier). Flag 0x4000000 uploads the fixed set
  0x7d6160 (712763, 712787).
- **A draw with 0x10 is UNLIT** — not "lit by the last uploaded set", as the card read it.
  - `DAT_007c5768` (the light packet) is written by `FUN_0057fd90`'s last statement (715405), zeroed by
    `… the ELF
    has three stores (0x5803cc, 0x580c68, 0x58b148).
  - `FUN_0058ed70` reads it (723394). With flag 0x10, with it 0, or with MTRL bit 0 clear, it clears bit 0x4000 in
    the copy of the segment's prim word it puts in the pass record (723489–723492), and it calls the light packet
    only when it is non-zero and 0x10 is clear (724027–724029).
  - Bit 31 alone would be unlit only if the packet is 0 on entry; the three skinned callers of `FUN_0057fd90`
    (`FUN_003c3520`, `FUN_003c6e60`, `FUN_003c8a30`) leave it set (verifier). No code that sets bit 31 was found
    (verifier's ELF scan) [H: unused].
- **Who passes them**: the 36 call sites of slot +0xc are tabled in `re-material-flags.md` §1 [V there, not re-read]:
  0x10 from the hologram's icon (`FUN_002186c8`) and the UI's 3-D models (0x4000091 / 0x4000081); 0x4000000 from
  `FUN_0037f110` / `FUN_0037fd30` and the UI.
- **A prop's flags.** The scene flush `FUN_005b0ab8` calls each queued object's vtable +0x54 with
  `(1.0, obj, 0, obj[+0x44] | pass)` (746873); pass = 0 for the near scene (698535, after the per-view light filing
  at 698531) and 0x60000 for the far scene (426027). For the prop entity (vtable 0x61ddb0; scene object at +0xd0,
  vtable 0x61dce0) the slot is `FUN_00230bc8` → `FUN_0022edd8`: it ORs 0x80 when the class byte +0x2e0 is 0
  (145486–145491) and calls the GameModel's draw `FUN_001c5610` (vtable 0x6187d8 + 8) with `flags | 0x1000000`
  (145495–145497), which passes them unchanged to slot +0xc (72102, 72120).
- **RAM, UTA1 cp3** (`pen_ram.py`): the pen entity 0x1baf3e0 has object flags 3 and class byte 0 → **draw flags
  0x1000083**. Its model (0x1b706a0) has +0x48 = 1 (prelit) and six segments, each with prim word 0x106000
  (lit | vc-lit | COL0).
- **RAM, the frame's sets** (`setpool.py`): the pool `FUN_0057fd90` tests at 715248 (0x7bb950, 128 × 0xa8) holds
  **24 sets with the prelit byte and exactly one omni, the post's light** 0x1bc5e20 ((1,86,213), r 8, 0.32 m over
  `con_cp1`). Prelit draws took the light.

## 2. The vertex term on the pen floor [V]

- **Disc** (`penterm.py`): segment 2 of `uta1_bldg_lizardpen` (`uta1_bldg_metalfloor02`, MTRL 0x10201) is a fan of 8
  triangles round vertex 132 at (−159.23, 10.40, 122.60), 0.35 m under the light, normal up, COL0 (29,31,33); the
  other fan vertices are 6.3–15 m away. `re-point-lights.md` §7's term gives vertex 132 0.976 (closed form; the VU's
  byte/128 normal and bit-trick 0.5/d give ≈ 1.015, verifier) and the rim ≤ 0.04.
- **The VU's inputs are what the port's are.**
  - Positions arrive as 128 + u16/65536 (row 0x43000000, `FUN_00494af8` 553802–553806) and are rewritten in place as
    VU[2] + (q − 128)·VU[3] (0x45a–0x47b): model-space metres. Normals arrive with row 0x47800080 (553872–553878);
    the loop subtracts 65537 (0x23f): byte/128.
  - 0x73f–0x757 moves the lights to model space: (position − VU[59]) × VU[7].y², projected on VU[56..58]; 1/r² ×
    VU[7].y²; VU[7].y = 1/|VU[56]| (0x3d5, 0x3e0–0x3e1), 1 for an unscaled matrix. It runs at draw time when the
    draw has VU flag 0x20 and a matrix or light upload set the dirty bit (0x3d3/0x3d9–0x3da, 0x59a–0x59b/0x5b5,
    0x61c–0x61f).
  - A vc-lit segment adds: colour = lit + material × (COL0 − 65536), clamped (0x537–0x549). The lit colour stays.

So the PS2 computes the same vertex colours the port does. The difference is what happens between the vertices.

## 3. The clipper [V][O]

- **Which batches.** `FUN_0058ed70` sets VU flag 0x100 from draw-flag bit 0 (724024–724030); VU1 sends a batch to the
  clip path 0x311 only then (0x1a5–0x1ad).
- **The volume.** The tests use the model→clip rows VU[44..47] with xyz scaled by VU[85].xyz plus VU[84].xyz·w
  (0x324–0x32d). `FUN_00580600` sends VU[85] = (width/4096 + 0.0001, height/4096 + 0.0001, 1), VU[84] = 0 (715577,
  715580, 715596 → `FUN_004976f8` → handler 0x3ad); six of its eight calls pass 512 × 448, the other two are a
  256 × 224 target behind `DAT_0060c230 == 0` and a split-screen height (verifier). BF1's twin `FUN_004997d0` has the
  same lines (B1 560188–560189) and BF1's VU1 the same tests at 0x331–0x343 [D: its clipper body not traced].
  - RAM (`vu_guard.py`): VU[85] = (0.1251, 0.109475, 1), VU[5] = (256, 224, 2²⁶), VU[4] = (2048, 2048, 2²⁶).
  - **So a triangle is clipped only beyond |x| ≤ 7.994 half-widths, |y| ≤ 9.135 half-heights** (2046 of the GS's
    2048 units each way) **or |z| ≤ w.**
- **Per triangle** (0x32f–0x348): all three vertices inside → left to the strip. Otherwise the strip vertex gets the
  no-draw bit 0x8000 and, unless all three lie beyond one plane of that volume scaled by VU[85] once more (63.9 ×
  83.4 half-screens), the triangle goes to the clipper 0x6c3.
- **The clipper** (0x6c3–0x6df, plane routine 0x666–0x6c2): Sutherland–Hodgman over six planes in the order of flag
  bits 0x10, 0x20, 8, 4, 2, 1. A cut vertex = inside + Q·(outside − inside) with ONE Q for position, texture
  coordinate and colour (0x6a1–0x6a6): exact where the edge meets the plane. The polygon is emitted as a strip taking
  vertices alternately from its front and its back (0x706–0x724).
- **Gouraud is kept**: the clipper keeps each vertex's colour when the pass's PRIM word has the bit of value 8 (IIP;
  0x311, 0x31d–0x31f, 0x34c, 0x355), else all three take the last strip vertex's (0x357). `FUN_0058ed70` puts 8 in
  the segment's own pass (723494–723496); the static fog pass record 0x7c5840 (PRIM 0x40) of far-scene draws lacks it
  (verifier).

## 4. The GS: colour is linear in screen space [D][V measured][O]

The GS interpolates RGBA along a triangle linearly in screen space; only ST/Q is perspective-correct. That is a
hardware property, not in the ELF [D]. The measurement:

- `gsraster.py` draws triangles through §3's clip (guard volume from the RAM values, plane order, inside-first cut,
  front/back strip) and blends affinely (`ps2`), or clips at the near plane and blends in perspective (`persp`).
  `uta_pred.py` draws the pen and its roof from the disc at the stored pose with the light's gain and COL0 per
  vertex. PS2 gain = control − the "no model omnis" knock-out (`lead_tmp/bpnt/frames/u3__*`; two controls 0.12
  apart); texel estimated from the knock-out frame.

  | box (640×480) | PS2 measured G/B | `ps2` | `persp` |
  |---|---|---|---|
  | x 2–35 %, y 80–98 % (the card's; overlaps the HUD) | +1.2 / +2.9 | **+1.2 / +3.0** | +20.3 / +48.7 |
  | x 110–224, y 436–470 (HUD-free) | +1.9 / +4.8 | **+1.9 / +4.5** | +29.4 / +70.2 |
  | x 68–98 %, y 84–98 % | +1.5 / +5.2 | +2.1 / +4.8 | +8.9 / +21.0 |
  | x 5–35 %, y 62–76 % | 0.0 / +0.1 | +0.1 / +0.2 | +1.3 / +3.1 |

  - No parameter is fitted. The verifier varied them: the prediction does not move with the near plane or the x
    guard and scales with the y guard, which is the RAM value.
  - Limits: green is over-predicted on the right half (unexplained); the `persp` column divides by its own darker
    COL0 map, which raises it — the port's rendered gain, +15.8/+38.1 (BPNT), is the firmer figure for that side.
- **COL0 goes the same way.** In the card's box the pen floor's COL0 is 0.80 through `ps2` and 0.55 through `persp`:
  the dark centre vertex spreads up the screen in perspective and stays below it on the PS2.
- **Why TAN1, KAM1, HOT1, TAT2 matched**: their lit surfaces are walls a few metres ahead or finely cut floors — no
  triangle with one vertex at the eye and the others far off, so both blends agree (TAN1's wall box and TAT2
  `cp3_cp1_b000` are unchanged by the prototype, §5).

## 5. The port prototype [V measured]

Scratch tree at 951e3e4, `scripts/parity.py run` (954 pairs), base and patched from one tree.

- **Plain `@interpolate(linear)`** on the colour and point varyings (`linear_color_point.patch`): COR1 −0.78,
  UTA1 −0.39, TAN1 −0.36 — and a black wedge on MYG1 `walk/cp4_cp5_con_b090`'s near floor (box 17.6 against the PS2's
  68.7, where `gsraster.py` gives all three modes within 10 %) and a white frame at the broken BES2 pose (+84.9). The
  GPU extrapolates across a triangle with a vertex behind the eye [D]. Not usable.
- **The safe form** (`affine_wclamp.patch`): carry a·wc and wc as ordinary perspective varyings and divide in the
  fragment stage. wc = max(w, |x|/7.994, |y|/9.1345, near) per vertex (x, y in half-screens × w): the vertex's depth,
  no nearer than where its view ray meets the guard band. Weights are β·wc (β the perspective barycentrics): the
  affine weights for a vertex in view, a convex mix always. In `uta_pred.py` (`wclamp`) it gives +1.2/+2.9,
  +1.8/+4.3, +2.0/+4.7, +0.1/+0.2 for §4's four boxes.
  - Applied to model draws (lit, prelit, prelit-add, unlit, unlit-add, character), rgb and the point sum; terrain,
    glow, water, sky and alpha left as they are.

  | | base | safe form | PS2 |
  |---|---|---|---|
  | all 954 pairs, mean block error | 10.672 | **10.639** | |
  | bf2_cor1 (30 pairs) | 6.745 | **5.945** | |
  | COR1 `walk/cp2_cp3` b000 / b090 / b180 / b270 | 5.99 / 6.67 / 6.96 / 7.00 | 2.46 / 3.10 / 3.44 / 3.53 | |
  | COR1 `cp2_cp3_b000` floor box (0,340,640,470) | 103.1/91.5/68.2 | 69.9/62.8/47.6 | 70.1/64.8/50.9 |
  | bf2_tan1 / bf2_myg1 / bf2_uta1 / bf2_hot1 | 7.525 / 12.412 / 7.863 / 10.289 | 7.048 / 12.228 / 7.718 / 10.164 | |
  | UTA1 `cp3_con_cp1_b000` card box | 65.5/81.1/101.9 | 86.0/86.8/86.7 | 72.3/77.0/78.9 |
  | TAN1 `cp0_cp6con_b270` wall (0,230,640,330) | 58.4/47.2/48.9 | 58.5/47.2/49.0 | 60.2/49.5/52.2 |
  | MYG1 `cp4_cp5_con_b090` box (416,384,627,470) | 75.4/74.9/74.7 | 65.9/65.4/65.2 | 68.7/68.6/68.4 |
  | bf2_kam1 (31 pairs) | 16.053 | 16.778 | |
  | bf1_bes2 (29 pairs) | 10.918 | 11.233 | |

  - UTA1's pen floor loses the pool (B − R +36 → +1; PS2 +7) and ends 8–14 levels above the PS2: the floor's own
    level, not the light. That pose's parity score rises 4.37 → 4.59.
  - **KAM1 rises over the parity gate's 0.5.** At `walk/cp2_cp3_b090` the PS2's deck is 82/84/89 at mid-floor
    (256,278,627,346); the port's is 45/48/51 before and 39/43/46 after. Near the camera (352,360,627,470) the port
    had 82/86/91 against the PS2's 80/84/93 — the post's pool, hiding a deck 1.8× too dark; after, 53/57/62. BF2
    Kamino's deck did not "match once BPNT landed" (card B1DK's wording): it is dark like BF1's.

## 6. DEA1: a post the mission kills [V][O]

- `dea1g_con` and `dea1c_con` call `KillObject("CP6")` and leave CP6 off the roster (the disc's luac through
  `scripts/gen_missions.py`; the source `…/scripts/DEA/dea1g_con.lua` 43–62).
- RAM (`dea1_cp6.py`, `bf2_dea1_walk_cp5_cp6_b000.p2s`; the same in all 14 DEA1 states, verifier): six non-Static
  r = 8 omnis on the render list, each 0.32 m over cp1, cp2, cp3, cp4, cp5, cp7; none within 54 m of cp6. A seventh
  light object sits at 0x1b799d0, 0.32 m over cp6, colour 0 and off the list.
- Port: `game::point_light::still_lights` lights every post of `MapLayout::from_manifest` and never applies the
  mission (`apply_mission` is called from `main/battle.rs:264` and three examples). A render with no battle lights
  CP6 — and, on DEA1, the campaign posts `cam_cp1`, `cam_cp2`, `cam_cp5` (verifier, read from the manifest) — with
  the world layer's owners, not the mission's. A battle (`point_light::submit`) reads the sim's posts.
- `walk/cp5_cp6_b090`, whole frame: PS2 24.2/32.3/41.8, port 21.8/40.2/67.8; the blend change does not move it.

## 7. What the port owes (card BPRL, build)

1. **`render/src/shader.wgsl`**: the model draws' vertex colour (rgb) and point sum blended as §5's safe form. The
   guard in view angles, so a wider window keeps the PS2's: |x_view| / (7.994 × 0.63707 = 5.093) and
   |y_view| / (9.1345 × 0.48838 = 4.461); near 0.1. A named deviation from perspective-correct shading, one const to
   switch off. Declares every golden it moves and `parity=bf2_kam1`.
2. **`game/src/point_light.rs`**: `still_lights` takes the posts of the map's conquest mission (roster, killed,
   owners), and `pose_render` the viewer's team from the pose (`team` in the pose JSON; card BPRL's note on HOT1 and
   KAM1).
3. Referee: `lead_tmp/bprl/gsraster.py` + `uta_pred.py` (the PS2's gain per box) and §5's table.

Notes for other cards: **B1DK** — BF2 Kamino's deck is as dark as BF1's (§5). **LFLR / BPNT** — nothing changes in
the gather or the term.

## 8. What stays [H]

- Whether terrain, water, sky and vertex ALPHA should take the same blend (the GS does it for every primitive; the
  port's meshes for those differ from the PS2's patches, not measured).
- At this note's baseline, lit models still shaded N·L per pixel. The 2026-11-04
  runtime correction evaluates the completed material lighting per vertex and blends it in display units:
  `re-vertex-lighting-port.md`. New port regressions cover this; matched PS2 acceptance remains separate.
- UTA1's pen floor 8–14 levels above the PS2 after the change; the green over-prediction on the right half.
- BF1's clipper body (same guard constants and tests; not traced).
- Bit 31 of the draw flags: no setter found.

## 9. Verification

**Self-checks** (`lead_tmp/bprl/checks1.txt`, `checks2.txt`, `checks_ram.txt`): `re_check.py fn` on every cited
line; `writers` DAT_007c5768 (3), DAT_0060c2b5 (0); `callers` FUN_00580600 (8), FUN_004976f8 (6), FUN_0058b140 (1),
FUN_0058b150 (4), FUN_005b0ab8 (1), FUN_005b06b8 (2), FUN_0022edd8 (1); `refs.py` 0x57cf60, 0x230bc8, 0x1c5610 (data
words only); `widescan.py` 0x7c5768; ELF constants 0x39800000 (1/4096) and 0x38d1…(0.0001) in `FUN_00580600`; the
twin `FUN_004997d0`; VU1 0x162–0x1e6, 0x210–0x2a1, 0x311–0x392, 0x3a4–0x408, 0x406–0x4c0, 0x520–0x56b, 0x598–0x640,
0x640–0x73e, 0x73f–0x757.

**The verifier** (one foreground Opus agent, claims only): **16 SUPPORTED, 4 PARTLY, 0 NOT SUPPORTED** of 20, all
folded in: A2 (bit 31 rests on the packet being 0; the 0x4000 clear is on the record copy; a third cause), C2 (six of
eight callers pass 512 × 448), C5 (PRIM bit of value 8; the fog pass record lacks it), D3 (three examples call
`apply_mission`; the campaign posts). It re-ran every script, re-read the RAM numbers with its own code, traced the
whole VU1 program (byte-identical to `vu1MicroMem.bin` in two states) and recomputed §5 from the score files. Its
note on reading VU1: a flag read or an integer load just ahead of a branch IS seen by the branch.
