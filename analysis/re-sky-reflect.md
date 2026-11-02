# OPUS RE (2026-09-29) - [O] UNAUDITED
# `Reflect` sky shells: the PS2 draws every Reflect shell twice more, mirrored under the eye — the sky under BF1 Kashyyyk's water (card REFL)

Card REFL, lane 2. Corpora: `analysis/SWBFRONT.ELF-decomp.c` (B1), `analysis/SWBFRNT2.ELF-decomp.c` (B2), the two ELFs
(disassembly `scripts/tools/eedis.py`), every `.LVL` on both discs, PS2 RAM from kept save states, and
PCSX2 frames with patched code and data (`engine/artifacts/lead_tmp/re/refl/ps2exp/`). Scratch and every script:
`engine/artifacts/lead_tmp/re/refl/`. Checks verbatim: `refl/checks.txt`. Verifier: `refl/verify/VERDICTS.md`.
Tags: [V] read or measured (cited), [D] derived (from what), [H] hypothesis. Load-bearing claims carry [O].

## §0 Answer
1. **What `Reflect` does.** For every `DomeModel` shell that carries `Reflect`, BF1's sky tick draws the whole shell
   TWICE MORE, before its normal draw, with one mirror matrix: X and Z unchanged, **Y × −(0.85 + 0.006·sin φ)**, origin =
   the shell's own (lerped) position **+ 0.1 m** in Y, tinted by the map's **`ReflectionColor`** [V][O]. It is a fake
   reflection of the sky about the eye's horizon, not a planar reflection about the water: nothing in it reads the
   water height. φ advances 2/30 rad per sky tick; the PS2 runs 29.9 ticks/s, so **2.0 rad/s** (period π s) [V][O].
2. **Where the two copies go** [V code + V PS2][O]. BF1 renders a near scene into the MAIN target and the sky / far
   scene into a separate FAR target, then blends far over main by an alpha computed from the main Z (1 where the main
   Z is empty or beyond `NearSceneRange.c`). The two mirrored draws feed the two targets:
   - flags **0x1000031** → segment bucket **3**, drawn into the MAIN target right after the main clear, BEFORE the
     terrain (which shares bucket 3), with no Z write. Everything opaque in the near scene paints over it; it survives
     only under a surface that is translucent AND writes Z — the NEAR WATER (bucket 8, ALPHA 0x44, TEST 0x50000). So the
     reflection seen IN the near lake is this copy, seen through the water's blend.
   - flags **0x20031** → bucket **9**, the FAR target, in submission order: mirrored shell, then the normal shell. It
     shows wherever the main Z is empty or far: below the horizon beyond the land, through lake-bed holes, and
     past the near water's reach.
   Both copies are the same picture, so on screen they join into one reflected sky below the horizon.
3. **On the PS2 itself** (KAS1 East Harbor lake, `ps2exp/means_v3dn.txt`) [V][O]: removing the 0x1000031 call
   (nop at 0x2f24f4) turns the near lake from (37.6,60.1,92.7) to (28.0,36.8,43.0) and leaves the band beyond it; removing
   the mirrored 0x20031 call (nop at 0x2f251c) leaves the near lake (37.5,60.0,92.7) and darkens the band beyond from
   (23.4,47.1,80.2) to (10.9,25.7,40.4). With no water at all the near lake shows the far copy bare, (15.2,42.1,94.8), and
   with the far copy removed too it is black (0.4,0.6,0.8). `ReflectionColor` poked to (255,0,0) turns the reflection red.
4. **Which maps.** `Reflect` is authored only on BF1 KAS1 (`kas1_sky_dome`, `kas1_mountains`, `kas_moon`) and KAS2 /
   KAS2_H (`kas2_sky_dome`, `kas2_mountains`); `ReflectionColor` only on KAS1, (200,200,255); default (255,255,255)
   [V disc]. `kas_moon` exists in no LVL, so its slot's model is 0 and the tick skips it [V RAM]. No BF2 map authors
   `Reflect` (BF2 has the same code, §6).
5. **Port rule** (§5, PROVISIONAL until built): in the sky pass, per shell in authored order, draw the mirrored copy of
   a `Reflect` shell, then the shell. A scratch prototype of that rule registers to the PS2's far target to −1 px (the
   known registration offset) and is pinned: squash 0.80 / 0.90 moves it −6 / +4 px, lift 0 / 0.2 m moves it +17 / −19 px.
   It removes REDB's white row on the KAS2 ground panel and fills KASF's Beach Head band: rows 128–136
   median (15,27,16) where the PS2 shows (11,21,12) and the port shows black today.

## §1 The data [V][O]
- B1 loader `FUN_002e29e0`: DomeModel key 0x92c778aa (fnv1a-32 of "reflect") → slot word `sky+0x34+0x14·n` = 1
  (270155–270156); DomeInfo key **0x8a81de8b = fnv1a-32("reflectioncolor")** (new preimage) → three bytes clamped to
  0..255 at sky+0x1c/+0x1d/+0x1e (270321–270339). Constructor `FUN_002e2888`: sky+0x1c..0x1f = 0xff (269946–269951), slot
  Reflect words 0; sky+0x100 = the word at 0x69c420 (269945), a .bss colour `FUN_00492598` fills with 0xff
  (555048–555049, 555070–555071). Nothing else writes sky+0x1c..0x1e (verifier: no store, no address use).
- B2 loader `FUN_003bcd50`: Reflect → byte slot+0x50 (416446–416447); ReflectionColor → sky+0x34..0x36 (416564–416585).
- Disc: a raw little-endian search of all 323 `.LVL` files and a `sky_` walk of the 51 that have one (`refl/census.py`,
  `refl/skykeys.py`): 0x92c778aa only in BF1 KAS1 (3×), KAS2 (2×), KAS2_H (2×); 0x8a81de8b only in KAS1:
  `DomeInfo ReflectionColor(200, 200, 255)`. `kas_moon` occurs once on the disc, as a string in KAS1's `sky_`.
- PS2 RAM (`refl/ramsky.py`): KAS1 sky object 0x1baf030 — 3 slots, slot 2 model 0, Reflect 1/1/1, sky+0x1c = c8c8ffff;
  KAS2 — 2 slots, Reflect 1/1, ffffffff. Also: composite gate 0x522b54 = 1, destination-alpha composite 0x522e84 = 1,
  16-bit far target 0x522ac0 = 1, one view (0x522ad8 = 1), no discarded bucket (0x522ea8..eb8 = 0xffffffff).
- The shells (baked GLBs, `redb/glbinfo.py`): the domes are ~4.4 m in radius (y 0..3.8, lowest ring +0.02°), the
  mountain rings 4.1 m (KAS1) / 2.9 m (KAS2), lowest ring +0.20°. MTRL words (the BF1 PS2 `MTRL` chunk is the flag
  word when bits 0x10/0x20 are clear): domes 0x1, mountains 0x5 → all Normal-class, mountains mode 2 (translucent).

## §2 The tick: two mirrored draws per Reflect shell (B1 `FUN_002f2308`, vtable 0x541588 slot +0x18 = 0x5415a0) [V][O]
```
phi += 2.0 * (1/30)                                    DAT_00518b1c (ELF 0.0), DAT_00518b20 = 2.0, 280194 (its only writer)
s   = 0.85 + 0.006 * sinf(phi)                        DAT_00518b24 = 0.006, FUN_00434218 = fdlibm sinf, 280210-280212
M   = [ (1,0,0,0), (0,-s,0,0), (0,0,1,0), (pos + (0,0.1,0), 1) ]      rows = images of X, Y, Z; row 3 = origin
for each slot n with a model:                           280227-280292
    pos = camera position; if lerp word: pos.y = cam.y + ((sky+0xec + Offset) - cam.y) * (1 - MovementScale) * S
    if Reflect:  draw(model, M, colour = sky+0x1c, 0x1000031)          280253   jalr 0x2f24f4
                 draw(model, M, colour = sky+0x1c, 0x20031)            280255   jalr 0x2f251c
    draw(model, Scale(S) with row 3 = pos, colour = sky+0x100 (slot 0) / 0x69c420 (others), 0x20031)   280273-280284
```
- Disassembly (0x2f2368–0x2f24f4, `checks.txt` §4): 1/30 = `lui 0x3d08 / ori 0x8889`; the UPDATED phase is stored
  (0x2f23c0) and passed in `$f12` to `jal 0x434218`; 0.85 = `lui 0x3f59 / ori 0x999a`; row 1 is `VSUB.xyz vf1, vf0, vf2`
  (= (0,−1,0)) then `VMULx.xyz vf1, vf1, vf2.x`, w zeroed by `sw zero, 28(sp)`; 0.1 = 0x3dcccccd added by `VADD.xyz`,
  w = 1.0 stored at 60(sp); flags `lui t0,0x100 / ori 0x31` and `lui t0,0x2 / ori 0x31`. M is built once per tick and is
  NOT scaled by S (S = 1 on the discs).
- The rate on a running PS2: φ read from RAM every 2 s for 12 s: 1.994 rad/s = 29.91 ticks/s (`ps2exp/phase.txt`); the
  verifier's EE-cycle count over four saved states: 1.9956 rad/s, whole ticks only.
- Angles at the Kashyyyk eyes [D, from the mesh radii and the placement]: the mirrored dome's upper edge is at
  atan(0.0987/4.4) = +1.3°; KAS2's mirrored mountains span +1.7° down to −11.9° (eye 2.5 m: lift (1−2.5)·0.005 m;
  ring 0.01 m at r 2.9, top ring 0.8 m at r 2.79), KAS1's mirror line sits +0.46° at its East Harbor eye (y −26.5,
  Offset −40, r 4.1). The normal dome starts at +0.02° and the normal mountains at about +0.05°, so the mirrored
  copy fills everything below.

## §3 Where the two copies land (B1) [V][O]
- **Segment draw.** All four shells are Normal-class, draw `FUN_004cf6f0`. A draw with 0x20000 goes to the sky variant
  `FUN_004d0108` (594247), which puts a 0x20-flagged draw in bucket 9 (`FUN_0049e220`, 594633) with TEST 0x51001 (594642).
  0x1000031 lacks 0x20000: F & 0x1000000 → `FUN_0049e1e0` (594262 → 594345) = `FUN_0049ee68(…, 3, …)` = **bucket 3**
  (562367). TEST 0x51001 under F & 0x20 (594347–594359) = no Z write; ALPHA 0x44 only when translucent — mode 2 (MTRL
  0x04), c.a ≠ 0xff or F & 0x400000; 0x48 if MTRL 0x80 or F & 0x400000 (594361–594365). F & 0x10 clears the lighting
  bit (594371–594372). The GS colour = seg+0x34 (white, 568926) × the passed colour / 255, halved to GS scale. No fog
  pass (single-texture program). So: dome opaque, mountains blended by texture alpha, unlit, unfogged, tinted by
  `ReflectionColor`, in both copies.
- **Bucket 3 is the terrain's.** Its only other users are the terrain near pass `FUN_00487ad8` (549139–549304) and patch
  draw `FUN_004857b8` (548047); 0x1000031 occurs once in B1. The sky is render list 0 at priority −9999 (269924, 269927);
  the terrain object 0x69c270 is list 2 at priority 0 (`FUN_00488778` → `FUN_00479f90(p, 2, 0)`, 549593; slot 0x558dc0 =
  0x487ad8); `FUN_00479458` runs lists 0..8 in order (540737–540741); per-bucket texture maps are cleared each frame
  (`FUN_0049b9a8` 561224–561232) and node packet lists are appended in submission order (`FUN_004a95f8`). So the
  reflection packets are the first in bucket 3; the terrain paints over them with Z. (The verifier walked lists 0 and
  2 in RAM.)
- **Buckets 9 / 10 / 0xd keep submission order**: `FUN_0049de40` (562264–562269) drops a texture's record unless it lies
  in the bucket's TAIL node, and `FUN_004a94d8` places the new record in the tail node (a new node only when the tail
  has no VRAM room). Every draw in those buckets lands in the tail node, so bucket 9 draws mirrored dome, dome, stars,
  mirrored mountains, mountains, in that order. Other buckets reuse a texture's record in whichever node holds it.
- **The frame** (`FUN_0049bf50`, per frame): `FUN_004a7098` (561455) queues the composite; `FUN_0049aa68(ctx, 0x6eeef0)`
  (561508) clears the MAIN target (FRAME 0x80000 = FBP 0 CT32, ZBUF 0x30000100; 567923, 567944) to colour 0 / Z 0 and
  leaves it bound; `FUN_004a8f18` (561545) kicks buckets 0..0x14 ascending (567296–567435; 0x522ea8..eb8, the discard
  set, are 0xffffffff with no writer). Order: **3** (mirrored shells → terrain), 5 (near opaque), **8** (near water:
  `FUN_0049e2e0` 562447, TEST 0x50000 = Z written, `re-water.md` §1), **9** (node 0x3de761c8 from `FUN_004a57e0` 565467:
  main Z → main-frame alpha through a CLUT, then the far target cleared and bound; then the sky pass and far scene),
  **10** (far water; then `FUN_004a7098`'s sprite: TEX0 0x664a23000 = the far target, ALPHA 0x54 = (Cs−Cd)·Ad + Cd
  when 0x522e84 ≠ 0, TEST 0x33001; 566041–566105, the branch taken when 0x522ac0 ≠ 0 — ELF and RAM 1, no writer).
  Gate `DAT_00522b54` (ELF 1, RAM 1): its only writer `FUN_004a8080` is toggled around special renders
  (`FUN_00173a98`, `FUN_002e5d60`, `FUN_002ef438`).
- **Consequence** [D from the above, V by §4]: per pixel below the horizon — near opaque → the geometry; near water
  (Z written, < `NearSceneRange.b`) → water over the bucket-3 copy; main Z empty or beyond `.c` → the far target's
  copy (with the far scene and far water over it); between `.b` and `.c` a blend of the two.

## §4 The PS2 itself (PCSX2; `ps2exp/`) [V][O]
One scripted run (`capture.py`, under ci.lock) from the library's KAS1 East Harbor spawn state: phase rate measured,
chase camera turned to the library's v3 heading (yaw 26.5°) and pitched to −19.3°, state saved, two live data pokes
with a frame each. `mkstates.py` makes patched copies (each patched word asserted against the ELF first; the verifier
diffed every state: only the claimed words differ); `run_ps2exp.py` boots each and saves its frame.

| variant | patch | near lake (100–140, 150–450) | band beyond (52–78, two side boxes) |
|---|---|---|---|
| ctl / ctl2 | — | (37.6,60.1,92.7) | (23.4,47.1,80.2) |
| no3 | nop 0x2f24f4 (the 0x1000031 call) | **(28.0,36.8,43.0)** | (23.2,46.6,79.5) |
| no9 | nop 0x2f251c (the mirrored 0x20031 call) | (37.5,60.0,92.7) | **(10.9,25.7,40.4)** |
| no39 | both | (28.0,36.8,43.0) | (10.7,25.3,39.6) |
| nowater | `DAT_00522a78` = 1 | (15.2,42.1,94.8) | (22.8,46.3,79.7) |
| nowater_no9 | + nop 0x2f251c | (0.4,0.6,0.8) | (10.1,24.4,38.6) |
| live red | sky+0x1c..1e = (255,0,0) | (40.2,36.4,42.7) | |
| live far-only | `DAT_00522e84` = 0 (plain far copy) | (14.9,41.8,95.0) | (22.6,46.2,79.7) |

Frames: `ps2exp/frames/refl_v3dn__*.png`, sheet `ps2exp/sheet_refl_v3dn.png`. The far-only frame IS the far target:
mirrored mountains and dome below the horizon, the LOWR sheet, a black hole where the soldier (nearer than the ~4 m
shell) failed the shared-Z test.

## §5 Port rule (PROVISIONAL; card REFL's build)
```
sky pass, per DomeModel shell in authored order (the far target's order):
  if shell.reflect:                                   (DomeModel key 0x92c778aa, no arguments)
      draw the whole shell with  T(eye + Y·((Offset − eye.y)·(1 − MS) + 0.1)·k) · Scale(k, −s·k, k)
      colour ReflectionColor (DomeInfo key 0x8a81de8b; default 255,255,255), the GS value (c+1)>>1 / 128; alpha 1
      the shell's own SkyMode (MTRL 0x04/0x02 → blend by texture alpha, else paint); no depth, no fog, unlit
  draw the shell as today
s = 0.85 + 0.006·sin φ,  φ = 2.0 rad/s × game time (φ = 0 at start → s = 0.85 in every headless golden)
```
- k is the port's one shell scale (`SKY_SHELL_RADIUS` / the first dome's radius); the 0.1 m lift scales with it, as
  the Offset lift already does — a uniform scale about the eye keeps the PS2's angles.
- The mirrored matrix has a negative determinant; the port's pipelines cull nothing.
- The port has no far target, so one sky pass stands for both copies. Below the horizon the bucket-3 picture and the
  far target's picture agree (the normal shells stop at +0.02°), and the port's near water (depth-written, blended)
  then shows the reflection wherever no terrain is drawn under it.
- **Prototype check** (scratch worktree, never committed; `refl/port/`, `refl/sheets/`, `refl/prototype.patch`): at the
  PS2 far-only pose (626×480 at the PS2's 1.3045 aspect) the prototype's sky pass (terrain and water dropped) registers
  to the PS2 far target at dy −1 / dx 0 px (NCC 0.56 in the mountain band; REDB found the same −1 to −1.5 px offset).
  Squash 0.70 / 0.80 / 0.90 / 1.00 → −17 / −6 / +4 / +14 px; lift 0 / 0.2 → +17 / −19 px. On main f56e5ed (before
  REDB's build landed) the base render equals both committed goldens byte for byte; with the prototype
  `maps/bf1_kas1.png` moves 299 px (11 vantage, 288 ground; max 72) and `maps/bf1_kas2.png` 787 px (ground only, rows
  119–121; max 181): row 119, the white haze row, (48.9,53.7,58.7) → (4.7,9.0,6.6); rows 120–121, black → the mirrored
  forest (19.0,28.8,18.6). KAS2 Beach Head `v1_sea` rows 128–136 (median): PS2 (11,21,12), port today (0,0,0),
  prototype (15,27,16).
- **What REFL alone does not change** (other cards): KAS1's lake shows its terrain bed until card TMSK drops the mask-0
  patches — with the prototype and the terrain dropped, the near-lake box is (44.2,66.0,95.3) against the PS2's
  (37.0,59.6,92.6), the port today (68.1,74.7,80.4). KAS1 authors `DisableLowRes`, so past `NearSceneRange` 150→180 m the
  PS2 lake is the far copy bare, (23,47,80); the port keeps drawing its water there (`OPENBF_FAR_CULL` off by default) —
  card FARC.

## §6 BF2 [V]
Same tick code in `FUN_003cb918` (ELF 0x632768 = 0x3cb918): phase `DAT_005ffd7c` += `DAT_005ffd80` (2.0)·(1/30) (425799),
s = 0.85 + `DAT_005ffd84` (0.006)·sinf (425816, `FUN_0052a878` = fdlibm sinf), Reflect byte +0x50 (425841), + (0,0.1,0)
(425842), colour sky+0x34 = BF2's ReflectionColor, flags **0x2000031** (425852) then 0x20031 (425854). `FUN_0058ed70`
sends F & 0x2000000 to bucket 3 (723416–723432) — BF2's near-opaque bucket; its order against BF2's terrain was not
traced [H]. No BF2 LVL authors Reflect, so nothing on BF2 shows it.

## §7 Corrections to older docs
- `re-sky-dome-placement.md` §3: "`DAT_005ffd80` = 2.0 and `DAT_005ffd84` = 0.006 are the time accumulator and
  sun-flicker constants, unrelated" is wrong: the accumulator is 0x5ffd7c, 0x5ffd80 its rate, 0x5ffd84 the mirror's
  wobble amplitude — the Reflect draw's parameters (and 425805 is a matrix copy). §8.4 "ignore until a map needs it":
  two maps need it (this doc). §9 "The BF1 draw path was not traced" is answered for the Normal class (§3 here).
- `re-fog-amount.md` §7: "the 0x1000031 draw's effect is not read [H]" → §3/§4 here.
- `re-sky-pass.md` §4: "composite sprite with TEST 0x33001 at 561641" is `FUN_0049c400`, a full-screen tint in bucket
  0xd; BF1's far-over-near composite is `FUN_004a7098` in bucket 10 (566041–566105).
- `render/src/sky.rs` (`Placement`) and `render/src/lib.rs` say Reflect is not built and "no baked map has been
  checked": both Kashyyyks author it.

## §8 Checks run (`refl/checks.txt`) and what stayed [H]
1. `re_check fn`: every B1/B2 line cited above (§1–§3, §6).
2. `re_check absent`: 0x1000031 (B1 1 hit, the tick; B2 0), 0x2000031 (B2 1, the tick), 0x8a81de8b and 0x92c778aa (the
   two loaders only), DAT_00518b1c/20/24 (the tick only), DAT_00522ea8 (the kick only), DAT_00522e84 (the composite only).
3. Twin: both ticks are vtable slots (0x5415a0 → 0x2f2308, 0x632768 → 0x3cb918); `callers FUN_0049e1e0` = 9 sites in
   3 functions; `FUN_004a57e0` ← `FUN_0049b9a8`; `FUN_004a7098` ← `FUN_0049bf50`; `FUN_00488778` ← `FUN_00481360`.
4. `re_check word`: 0x518b20/24 = 2.0/0.006, 0x518b1c = 0.0; 0x522ea8..eb8 = 0xffffffff; 0x522e84, 0x522b54, 0x522ac0,
   0x522ad8 = 1; 0x558dc0 = 0x487ad8; the tick's instruction words (0x2f2368…0x2f251c); B2 0x5ffd80/84 = 2.0/0.006.
   0x69c420 is .bss (filled by `FUN_00492598`; RAM ffffffff).
5. `re_check writers`: 0x518b24, 0x518b20, 0x522e84, 0x522ac0, 0x522ea8..eb8, B2 0x5ffd80/84 — none; 0x518b1c — the
   tick only; 0x522b54 — `FUN_004a8080` only.
- Stayed [H] / not read: BF2's bucket-3 order against its terrain; the Scroll / Specular / Blur segment classes'
  0x1000000 handling (no Reflect shell uses them); the 16-bit far target's quantisation of the far copy (a hardware
  limit the port does not copy); the port's near/far split (FARC).

## §9 Verifier
One foreground Opus verifier on C1–C18 (`refl/CLAIMS.md`), given the claims and citations only; its own disc, RAM, PNG
and ELF readers are in `refl/verify/`. **14 SUPPORTED, 3 SUPPORTED with a caveat (C10, C11, C15), 1 PARTLY (C13),
0 NOT SUPPORTED.** Folded in above:
- C13 PARTLY: the ordered-bucket rule was misdescribed ("a new node per texture change"); it is "the texture's record
  must lie in the tail node", with the same conclusion (§3). Reproduced by the lead from `FUN_004a94d8`.
- C10: ALPHA is 0x48, not 0x44, when MTRL 0x80 or F & 0x400000 (neither applies to the Kashyyyk shells).
- C11: the 0x69c420 fill lines are 555048–555049 / 555070–555071; the 4-byte MTRL holds only without bits 0x10/0x20.
- C15: FRAME 0x80000 is set at 567923; the composite described is the `DAT_00522ac0 ≠ 0` branch (ELF and RAM 1).
- C18: confirmed, and `re-sky-dome-placement.md` §3's label is wrong (§7 here).
- Its extras: the EE-cycle rate (§2); three uncalled, undecompiled functions (a bucket-3 two-texture wrapper at
  0x49ec38, a sky getter 0x2e3f58, a sky+0x100 setter 0x2e3f70) that change nothing here.
