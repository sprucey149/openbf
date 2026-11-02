# LEAD VERIFICATION (Fable lead, 2026-09-03, session 12): re-read before acceptance — `FUN_0057b850` 711660–711674 (halfW = +0x138 · 2·near / S), `FUN_00580728` 715603–715650 (X scale = 1/argument 1, far = argument 4), `.data` 0x60c288 = 10000.0, the flush `FUN_005b06b8` 746694–746711, the zoom lerp 64893–64896, `FUN_00589fa8` 720830–720850 (far target `{0x800000000a080180, 0x30000000}` when `DAT_0060c230` ≠ 0; 0x60c230 = 01, 0x60c250 = 01 in the ELF), the composite 719905–719925 (ALPHA 0x54 when `DAT_0060c4e8` = 01, TEST 0x33001), and the sky TEST selection 723449–723480 (with the draw flag 0x20, material bit 0x02 takes the ALPHA path and TEST 0x51001). BF1: the sky variant `FUN_004d0108` 594636–594648 gives a mode-1 (0x02) shell BOTH ALPHA 0x44 and TEST 0x5100b with AREF `DAT_00523024` = 0x40 — BF1 blends AND tests, BF2 only blends; no BF1 sky shell on the discs carries 0x02 (session-11 census), so the port's rule is the BF2 one. Tags and text below are the RE agent's.

# RE: the sky pass — camera scale `S`, projection, Z, and where the dome ends up (2026-09-03, Fable s12, analysis only)

Corpora: `analysis/SWBFRNT2.ELF-decomp.c` (BF2, line numbers below are its lines unless marked BF1),
`analysis/SWBFRONT.ELF-decomp.c` (BF1), raw ELFs via `scripts/tools/elfrd.py`,
one Ghidra listing `engine/artifacts/lead_tmp/s12/re/skyproj_scratch/disas_58efa0.txt` (BF2 0x58efa0–0x58f060).
Tags: [V] verified in decomp lines / ELF bytes, [D] derived from [V] facts, [H] hypothesis.
GS register words decoded with the PS2 GS layouts (TEST: ATE b0, ATST b1-3, AREF b4-11, AFAIL b12-13, ZTE b16, ZTST b17-18;
ZBUF: ZBP b0-8, PSM b24-27, ZMSK b32; FRAME: FBP b0-8, FBW b16-21, PSM b24-29, FBMSK b32-63; ALPHA: A,B,C,D 2 bits each).

## §0 Headline / verdict

1. **The shell's angular geometry on the PS2 is exactly the authored mesh about the eye.** Nothing in the sky pass
   alters it: `cam+0x140`/`cam+0x90` = `S` is the camera's ZOOM factor (it divides the projection's tan(fov/2)),
   not a world-distance multiplier; the far-scene pass has NO projection of its own — one projection per frame,
   near = camera near (0.5 m on MUS1), far = 10000 m fixed; the pass's `[80·S, 2000]` reaches only the CPU
   sphere-cull record and the far-scene queue walker, never VU1/GS. No clip plane touches a 4–20 m shell. [V]
2. **Depth: the sky is drawn with GS TEST 0x51001 = Z-test GEQUAL but NO Z WRITE, forced by the draw flag 0x20
   for every material** (opaque dome included) — BF2 `FUN_0058ed70` 723464–723465 + Ghidra bytes; BF1 `FUN_004b0c48`
   572592–572600. It is the first thing in the far-scene bucket, painted into a separate 16-bit colour target that
   shares the main Z buffer (shipped mode); far-scene objects (≥ 80·S m) overdraw it Z-tested; the far target is then
   composited over the near scene with per-pixel alpha = f(near-scene depth) (1 beyond 110 m or empty, 0 nearer than
   80 m). So the sky is behind everything by construction; its own 20 m depth never enters the Z buffer. [V/D]
3. **The port's "shells scaled to 1000 m about the eye, depth test off, drawn first" reproduces the PS2 picture**
   for angles and ordering. What it does NOT reproduce: (a) shells with MTRL bit 0x02 (mus1_sky_volcano) are
   alpha-BLENDED (ALPHA 0x44), not alpha-tested — the prior report's "AREF 0x60 cut" is wrong for sky draws; (b) the
   far target is cleared to black and is a 16-bit, bilinear-magnified, possibly field-height buffer (PS2 sky is softer);
   (c) the 80–110 m near/far blend band is a per-pixel depth LUT, not a per-object fade. [V/D]
4. `S` ≠ 1 does happen on the disc: the player-camera controller lerps it to the controlled entity's zoom
   (scope zoom, BF2 `FUN_001bb510` 64896; BF1 `FUN_001a9f20` 52350). It changes the FOV, the far-scene split (80·S)
   and the dome's scale together — angle-invariant for the sky. The port may ignore it. [V for the writer, D for effect]

## §1 What `cam+0x140` (BF2) / `cam+0x90` (BF1) is — question 1

**A projection zoom divisor.** [V]
- `FUN_0057b850` (711660–711672): out = {near = cam+0x130, far = cam+0x134, halfW = cam+0x138 · (2·near / S),
  halfH = cam+0x13c · (2·near / S)} with S = cam+0x140 (711669–711674).
- `FUN_0057b8b0` (711686–711700): cam+0x138 = `FUN_0052a968(fov·0.5)` [D: tan], cam+0x13c = that · aspect;
  `FUN_0057b990` (711739) sets near/far only; all three call `FUN_0057a920`.
- `FUN_0057a920` (711001–711030): cam+0x144 = cam+0x138 / S, cam+0x148 = cam+0x13c / S, then the secants
  (`FUN_0052bc10(x²+1)` = sqrt) and a 7× wider set at +0x15c/+0x160 — the cull-cone tangents used by
  `FUN_005ae940`'s "7× wider planes" (745256) [V].
- The GS projection is built from those extents: every `FUN_00580728(halfW·0.5/near, halfH·0.5/near, near, 10000, ctx)`
  call (716863, 718359, 719424, 719429, 719876, 719983, 720010, 720886) passes `halfW·0.5/near = tan(fov/2)/S`, and
  `FUN_00580728` (715603–715660) stores *(decompiler detail omitted)* as the X scale (DAT_007bb540), `S/(tan·aspect)` as Y
  (DAT_007bb554), Z row `(n+f)/(n−f)`, `−2nf/(n−f)` (715642–715645), uploaded by `FUN_00497800`. So S multiplies the
  projection scale = zoom in. [V]
- `FUN_0057ab20` (711079): `S > 1.01 ? 16.0 : 4.0` — a zoomed-vs-normal constant handed to a caller ([H] aim cone). [V]

**Writers of S** (setter `FUN_0057b890(value, cam)` 711681–711686 → `FUN_0057a920`): [V]
- `FUN_001bb510` (64492, the player camera controller): no controlled entity → `S = 1.0` (64545); otherwise
  `target = 1.0; entity->vt[+0x1c][+0x14](entity, &target)` (64748–64750) then
  `S = target + (S − target) · FUN_0052b1b8(−25·dt)` (64893–64896) — a first-order lerp toward the entity's zoom,
  and the FOV lerps the same way (64899). BF2 parses weapon `ZoomMax` 0x2a96e8b4 / `ZoomMin` 0x1881b1a6 /
  `ZoomFirstPerson` 0x14ec7e1d (320352/320430/320422) [V]; the chain from those to the vcall was not walked [H].
- `FUN_001bc5a0` 65312, `FUN_001c54b0` 71994, `FUN_001cd5c0` 76952: `S = 1.0` (other camera modes).
- Camera-slot init stores 389111 / 389251 / 389357 / 389476 (`FUN_003912c0` region, 0x150-byte slots): 1.0.
- No other float writer of a `+0x140` in the decomp is a camera (the rest are timers/counters; 302928, 323441, 340602,
  417993 are different structs by context) [V, grep of `(float *)(… + 0x140) =`].

**Readers of S**: sky tick 425784; flush `FUN_005b06b8` 746694 (near-scene far = `DAT_00746020·S`) and 746704
(far-scene near = `DAT_0074601c·S`, gated on `DAT_00746028`, default 1 at 698062, set by sky_ key 0xc15d0860 →
`FUN_00565150` 416988, preimage unknown); far walker `FUN_005b0330` 746624 (same near·S); temp-camera builders
706506 / 706863 (`FUN_0057b990(80·S, …)` on a stack camera copy for the 0x20000 pass of `FUN_00571bd0`/`FUN_00572458`
— the LowResTerrain/far-terrain path [H]); 485398, 696092, 686761, 535560 (screen-size ↔ world-size conversions,
all divide by `2·near·S`-style terms, consistent with zoom) [V for the sites, D for the reading].

**Verdict for Q1**: S is a projection zoom; on the disc it is 1.0 except while a zoom lerp is active. Scaling the dome
mesh by S (425877) and the far split by S keeps the far-scene cull consistent under zoom; for the sky it is
angle-invariant either way. [D]

## §2 The far-scene pass: projection, Z, targets, composite — question 2

### 2.1 One projection per frame; the pass ranges are CPU-only
- `FUN_005b06b8(queue, flags, cam)` (746672–746760): near-scene flush (`flags & 0x40000 == 0`): near = cam+0x130,
  far = `DAT_00746020·S`; far-scene flush: near = `DAT_0074601c·S` (if `DAT_00746028`), far = `DAT_00746024`,
  return if far < near (746696–746711). Both go ONLY to `FUN_005ae940(near, far, 0x7ec470, cam)` (746714/746719) and
  to `DAT_007ec560..574` (746734–746741: near, far, tan/S, cos — cull helpers). [V]
- `FUN_005ae940` (745226–745262): record[0x21] = near, [0x20] = far, eye at [0], −forward at [0x1c], six planes from
  `FUN_0057b850`'s extents and a 7× set. Readers of the record: `FUN_005aef98` 746019, `FUN_005aecc8` 746250,
  `FUN_005b0330` 746638, `FUN_005b0c70` 746928 — all CPU sphere/frustum culls. No VU1/GS consumer of [0x20]/[0x21]
  was found (grep `DAT_007ec4f0|DAT_007ec4f4|0x7ec470`). [V negative]
- The GS/VU1 projection is set by `FUN_00580728` with *(decompiler detail omitted)* (the camera near, 0.5 on MUS1: `FUN_00564f50`
  698737–698749 clamps `NearSceneRange[0]` by the Z-precision formula and `FUN_0057b990(DAT_00746018, DAT_00746020, cam)`
  at 698750/698529/698569 pushes near=0.5, far=110 into every camera slot) and *(decompiler detail omitted)*
  (ELF .data 0x60c288 = 0x461c4000; also written at 721350 and 715918). No call passes 80 or 2000. [V]
- `FUN_00580600(ctx, W, H, 0x20)` (715557–715600): viewport scale W/2, H/2, Z scale/offset `1 << (0x20−6)` = 2^26. [V]
- Z mapping [D from the two above + `FUN_0058ae10`]: Z_gs(z) = 2^31 · (n/(F−n)) · (F/z − 1), n = 0.5, F = 10000,
  into a PSMZ32 buffer (ZBUF PSM field 0 everywhere, e.g. 722660 `DAT_007c5138 = 0x30000000`). `FUN_0058ae10`
  (721340–721410) computes exactly this for z = 80 and z = 110 (*(decompiler detail omitted)*, *(decompiler detail omitted)*,
  721350–721365) — the EE mirror of the VU1 mapping. Values: z=0.5 → 0x80000000, 20 → 0x03319a3c, 80 → 0x00cb2bf8,
  110 → 0x0093507d, 2000 → 0x00068dce, 10000 → 0. Z decreases with distance; ZTST=GEQUAL (2) = "nearer or equal passes";
  clear value 0 = infinitely far.
- So the far-scene "near" (80 m) is neither a clip plane nor a Z remap: it is (a) the CPU cull threshold and (b) the
  queue walker's rule — `FUN_005b0330` (746531–746660): pop the far queue (heap by distance), cull if screen-size
  `r²/d² < k` or `d − r ≥ far` (746643), draw opaque objects only if `near·S < d − 2r` (746646–746648, flags | 0x20000),
  and objects with material bit 2 with a fade `(d − near·S)·0.05` clamped to 1 (746651–746656). [V]

### 2.2 Render targets and buckets (BF2)
- Bucket assignment in `FUN_0058ed70` (723411–723443): flag 0x20000 → bucket 8, or 9 if translucent and the sky flag
  0x20 is clear; near scene → 3 (opaque), 0xb/0xc/0xe (translucent classes); reflection 0x4000000 → 0x12. Packets go to
  the bucket's display list via `FUN_00584c30(bucket, …)` 723961/723967 → `FUN_00585580` (718367) → `FUN_00584618`
  (717647): buckets 8, 9 and 0xc get their own target node (`bucket − 8 < 2 || bucket == 0xc`, 717665–717669,
  `DAT_0060c231 = 1`, `FUN_00582c90` 716969 allocates). [V]
- Kick order: `FUN_0058a598` (721041–721200) walks buckets 0..0x15 ascending, nodes in creation order
  (`FUN_0058ab00` 721210–721230 appends at the tail), calling `FUN_00589fa8(ctx, bucket, view)` before each node's
  items. [V]
- `FUN_00589fa8` (720802–720900): for buckets 8/9 binds FRAME/ZBUF/SCISSOR/XYOFFSET =
  `{0x80180, 0x300001a0}` (FBP 0x180, FBW 8, CT32; own Z at ZBP 0x1a0) when `DAT_0060c230 == 0`, else
  `{0x800000000a080180, 0x30000000}` (FBP 0x180, FBW 8, **PSMCT16S**, FBMSK alpha; **ZBUF ZBP 0 = the main Z buffer,
  ZMSK 0**) (720837–720843), scissor 0..511 × 224 rows (720837); every other bucket binds `{0x80080, 0x30000000}` (main frame FBP 0x80, main Z). It returns
  immediately when `DAT_0060c250 == 1` (single view). ELF bytes: `DAT_0060c230` = 0x01, `DAT_0060c250` = 0x01
  (elfrd 0x60c230 / 0x60c250) and neither has a writer in the decomp (`DAT_0060c230`: reads only at 719391/719421/
  719432/719883/720848). **So the shipped mode is: far target = FBP 0x180 16-bit colour, sharing the main Z.** [V]
- Single-view binding path [D]: `FUN_00564638` (698415, the per-frame scene function) calls `FUN_00582248` (698521)
  → `FUN_00587278` (716648 → 719138) BEFORE the tick loop (698540) that runs the sky tick (`FUN_00563a20` on the
  `&DAT_00745ee0` lists; the sky registers there via `FUN_00563960` → `FUN_005652f0` 698920–698936, priority −9999).
  `FUN_00587278` creates the bucket-8 node named 0x3de761c8 (719296, `FUN_005849f0(0x3de761c8,0,0)` = bucket 8) whose
  packets: (i) bind the main target (719310 `FUN_004978d8(ctx, 0x7c5120)`) and run 0x70 strips that read the main Z
  through the 16×16 8-bit CLUT texture 0x3de761c8 (built by `FUN_0058ae10` 721383–721400 / `FUN_00584838` 717729–
  717770: entries [0, Z(110)>>16] = alpha 0x80, ramp down to 0 at Z(80)>>16, 0 above — i.e. alpha 1.0 for pixels beyond
  110 m or empty, 0 for pixels nearer than 80 m), writing that alpha into the main frame (719313–719386: COLCLAMP 0 / TEST 0x31001 / ALPHA 0x48, strips,
  a PRIM 0x156 pass, then the per-strip FRAME/ZBUF/TEX0 loop 719370–719386); (ii) clear the far target: `DAT_0060c230 == 0` → `FUN_00581830(ctx, 0x7c4f90)`
  (colour + Z clear, TEST 0x30000, RGBAQ 0, Z 0; 716209–716260) else `FUN_00581ef0(ctx, {0xa080180, main Z})` +
  `FUN_00581a98` (colour-only clear, TEST 0x31001, 716409ff / 716283ff — the shared main Z is NOT cleared);
  (iii) set the viewport 512×448 (719426–719429 in the shipped branch) and the frame projection (`FUN_00580728`, cam
  near, 10000); (iv) `FUN_00589fa8(ctx, 8, 0)` (719431, a no-op in single view). Because this node is created first
  each frame and nodes are freed at kick end (721160–721190), the sky/far node created later by the tick inherits the GS
  FRAME/ZBUF state it leaves: far colour target + main Z. [D]
- Frame end `FUN_005828d8` (716784–716905): `FUN_00588968` (716814 → 719809) pushes the far-over-near composite into
  **bucket 9** (719870 `FUN_00584a10(0,0,0)`; binds main target 719872; viewport 512×448 719877; shipped branch
  719910–719956: textured sprite PRIM 0x15e, **ALPHA = 0x54 = (Cs − Cd)·Ad + Cd (destination alpha) when
  `DAT_0060c4e8` ≠ 0 (ELF 0x60c4e8 = 0x01; no writer in the decomp) else 0 (plain copy)**, **TEST 0x33001 = ATST NEVER /
  AFAIL RGB_ONLY / ZTST ALWAYS (no Z, no alpha write)**, RGBAQ 0x80808080, TEX0 = `DAT_007c5010` = 0x664a23000 = TBP
  0x3000 (= FBP 0x180), TBW 8, CT16S, 512×512, then 16 vertical 32-px strips with UVs from `DAT_0060c4e0/4e4`,
  Z=0xffffffff, 719933–719956); then the main colour+Z clear for the next frame
  (716850–716869, descriptors 0x7c5150/0x7c5120); then the kick `FUN_0058a598` (716902). [V]

### 2.3 Resulting frame order [D from 2.2]
bucket 3 (near opaque, main target, Z written) → … → bucket 8 node "0x3de761c8" (main Z → main-frame alpha via the CLUT;
far colour target cleared to black; far target bound with the main Z) → bucket 8 sky/far node (sky first, TEST 0x51001;
then far objects with normal TEST/Z write) → bucket 9 (far translucent, then the composite: far target over main frame
weighted by the LUT alpha) → buckets 0xb/0xc/0xe… (near translucent, effects, HUD).
Z is never cleared between the near and far passes in the shipped mode; the far pass Z-tests against the near scene's Z
and adds its own. The far-scene near (80 m) is a cull/blend distance only.

## §3 Does anything scale or push out the DOME MODEL — question 3

- Sky tick `FUN_003cb918` (425708–426030): re-read. Model matrix = Rotation × Scale(S,S,S) with row 3 = pos
  (425856–425883); no other translation/projection/viewport call between the draws; `FUN_005849f0(0,0,0)` at 426020
  only re-selects bucket 8's default node; `FUN_00560540(0x20000)` → `FUN_005755b0` (708251) is the far-terrain/
  LowResTerrain queue; `FUN_00589e38(0)` clears `DAT_0060c2a8` (ELF 0x60c2a8 = 01) — read at 727615 to route a
  different draw between buckets 9 and 0xc while the sky is queued [V for the site, H for meaning]. No 0x20000/0x20031
  scale, translation or projection override. [V negative]
- `FUN_0057cf60` (712624): the only 0x20000-conditional code is the per-segment sphere cull radius (712728–712732,
  far-scene far `DAT_00746024`, else −1) and the distance-fade skip when `(flags & 0x4020000) != 0` (712818–712828).
  The sphere test `FUN_0057bc50(radius, far, …)` passes for an eye-centred sphere. No position rescale. [V negative]
- `FUN_0058ed70` (723257): the sky flag 0x20 changes exactly two things — (a) bucket 8 even when translucent
  (723437–723438); (b) the GS TEST word, 723445–723480, confirmed by the Ghidra bytes:
  `0058efb4 andi v0,s5,0x20 / beq → f02c; 0058efc0 lw v0,0x18(s3); andi 2; beq → f024 (delay: andi v0,s5,0x20);
  0058f024 bnel v0,zero,f048 (delay: ori s7,s7,0x1001)` with s7 = 0x50000 (0058efa4 `lui s7,0x5`). So:
  - sky flag + material bit 2 clear (mus1_sky_dome 0x10001/0x10010): ALPHA 0 (no blend), **TEST 0x51001** (ATE=1,
    ATST=NEVER, AFAIL=FB_ONLY → colour only, **Z not written**; ZTE=1, ZTST=GEQUAL);
  - sky flag + material bit 2 set (mus1_sky_volcano 0x10003): takes the translucent path 0058efd0: **ALPHA 0x44**
    (Cs·As + Cd·(1−As)), colour alpha × material alpha (723449–723450), **TEST 0x51001** — blended, not alpha-tested;
    `0x5000b` (AREF test) is only reached at 0058f03c on the non-sky path (723477–723479).
  The AREF field `DAT_0060c64c << 4` is ORed in at 723553/723598 but ATST=NEVER makes it inert. [V]
  This corrects `analysis/re-sky-dome-placement.md` §0.5/§5 and the RE_LOG entry: for SKY draws the material rule is
  "bit 0x02 or 0x04 or alpha≠0xff → blend 0x44 (0x48 with bit 0x40, 0x64 with bit 0x08); else opaque", always without
  Z write. The general (non-sky) rule in the same function is unchanged.
- VU1 upload: the segment packet header built at 723600–723620 carries ALPHA | flags<<32 | prim word and the TEST word;
  the VU1 program is not disassembled here; the projection it uses is the frame's (§2.1). [V for the header, H that the
  microprogram adds no Z bias — none of the EE-side Z globals `DAT_0060c27c..294` is read by the model path; their only
  readers are `FUN_00589d60`/`FUN_00589dc8` 720672–720692, fraction→distance converters].

## §4 BF1 equivalents — question 4

- `cam+0x90` is the same zoom divisor: `FUN_00491008` (554177–554190) = `FUN_0057b850` twin (near +0x80, far +0x84,
  halfW = +0x88·2·near/S); `FUN_00490268` (553651–553680) = `FUN_0057a920` twin (+0x94 = +0x88/S, +0x98 = +0x8c/S);
  setter `FUN_00491048` (554196–554199). Writers: `FUN_001a9f20` (51978, camera controller): 1.0 at 52029 (no entity),
  lerp `S = target + (S − target)·exp(−25·dt)` at 52350 with target from `entity->vt[+4][+0x10]` (52038–52039);
  1.0 at 52706 (`FUN_001aa…`), 53656. Other `+0x90` float stores (51729, 207914, 209063, 209098, 237162, 251113, 252023,
  313280, 370307, 580695) are non-camera structs by context [D — pointer names/other fields differ]. [V]
- Projection: `FUN_004998e0` (560213–560260) = `FUN_00580728` twin; every caller passes `DAT_00522b0c` = 10000
  (ELF 0x522b0c = 0x461c4000; 561504, 562981, 565599/565604, 566042) and the camera near from `FUN_00491008`. [V]
- Ranges: `FUN_00479c10` (540916–540945) = `NearSceneRange` → `DAT_0067765c` (clamped near), `DAT_00677660` (80),
  `DAT_00677664` (110), pushes near/far into the four camera slots (`FUN_00491148`) and calls `FUN_004a97d0`
  (567625–567700, the Z/CLUT twin, same 78.125/(10000/mid − 0.9921875) formula, texture 0x3de761c8 at 567687);
  `FUN_00479cf0` (540946–540956) = `FarSceneRange` → `DAT_00677668`. Defaults 72/120/2000 at 540538–540541. [V]
- Flush `FUN_004c6608` (587412–587470): far pass near = `DAT_00677660 · cam+0x90` UNCONDITIONALLY (no `DAT_00746028`
  gate, 587439–587442), far = `DAT_00677668`; `FUN_004c4ac0(near, far, 0x701390, cam)` (frustum record, twin of
  `FUN_005ae940`); walker `FUN_004c62b0` (587281–587411): same cull `near·S < d − 2r` for opaque (587388–587392), fade
  for bit-2 objects (587394ff). [V]
- Tick `FUN_002f2308` (280127–280305): S read at 280180, dome scale `FUN_00446590(S,S,S)` at 280273, no other override;
  flush `FUN_004c6608(DAT_00522fa8, 0x60000, 0)` at 280300. [V]
- Segment draw `FUN_004b0c48` (572486): bucket select via `FUN_0049e220/…` (572571–572590; 0x20000 → `FUN_0049e220`);
  *(decompiler detail omitted)* (572592–572596) — **the same
  "sky flag → no Z write" rule**, and the blend word 0x44 only when translucent (572600). [V]
- Targets: `FUN_004a8890` (567045–567125) = `FUN_00589fa8` twin; BF1 far buckets are 9/10 (567105–567106) →
  `{0x80180, 0x300001a0}` (567109–567110, own Z) or, when `DAT_00522ac0 != 0` (gate 567119; ELF 0x522ac0 = 0x01),
  `{0x800000000a080180, 0x30000100}` (567111–567114) = 16-bit far colour
  at FBP 0x180 sharing BF1's main Z at ZBP 0x100. Named node `FUN_0049e220(0x3de761c8,0,0)` at 565467; own-Z clear
  `… composite sprite with TEST 0x33001 at
  561641. So BF1 has the same structure. [V for the sites; D that the frame order matches BF2 — not walked]
  **CORRECTED 2026-09-29 (card REFL, `analysis/re-sky-reflect.md` §3 [O]):** the TEST 0x33001 sprite at 561641 is
  `FUN_0049c400`, a full-screen tint in bucket 0xd. BF1's far-over-near composite is `FUN_004a7098`, in bucket 10
  (566041–566105): TEX0 0x664a23000 (the far target), ALPHA 0x54 when `DAT_00522e84` ≠ 0, TEST 0x33001. The BF1 frame
  order is now walked there (§3): main clear, then buckets 3, 5, 8 (near water), 9, then 10 (far water, then the
  composite).

## §5 What the port reproduces, and what it does not

Reproduced by "shells at k·mesh about the eye, depth test off, drawn first" [D]:
- Angles: identical (uniform scale about the eye; PS2 S is a zoom, dome scale × S is angle-invariant).
- Ordering: identical in effect — PS2 sky has no Z write and is overdrawn by everything the far pass draws and by the
  near scene through the LUT composite; "behind everything" is exact except in the 80–110 m band (below).
- Near clipping: none on either side (PS2 near plane 0.5 m against a ≥ 4 m shell).

Not reproduced:
1. **Blend rule for sky materials** (§3): dome (bits 0x01/0x10) opaque; volcano (bit 0x02) blended 0x44 with its texture
   alpha, NOT alpha-tested at 0x60. The port's always-composite sky pipeline is right for the volcano and harmless for
   the dome (alpha 0xff texels); the prior recommendation "draw bit-2 shells alpha-tested" must be dropped.
2. **Background where no shell covers**: the far target is cleared to RGBA 0 (TEST 0x30000 / RGBAQ 0 at 716224–716243; colour-only twin `FUN_00581ef0`),
   so uncovered sky pixels are black on the PS2, not the near clear colour (clear packet 716224–716243).
3. **The 80–110 m band**: per-pixel LUT alpha on the near scene's depth (`FUN_00584838`), so near geometry between 80
   and 110 m fades INTO the far target (sky/far objects) rather than a per-object alpha; the port has a per-object fade.
   Sky-visible only where the near scene is empty or beyond ~110 m.
4. **Resolution/format**: far target is PSMCT16S, 512 px wide, 224-row scissor per view in the split path
   (720837), bilinear-magnified (TEX1 0x120, MMAG=1) into 512×448 — the PS2 sky is 16-bit and soft. The exact
   single-view height is not closed (§Not closed).
5. Zoom: under scope zoom S>1 the far split moves to 80·S and the sky mesh scales by S; no visible sky change.

## Reproduction

```
cd <repo>
# S = zoom divisor
sed -n 711660,711700p analysis/SWBFRNT2.ELF-decomp.c        # FUN_0057b850 / FUN_0057b890 / FUN_0057b8b0
sed -n 711001,711030p analysis/SWBFRNT2.ELF-decomp.c        # FUN_0057a920: +0x144 = +0x138/S
grep -n "FUN_0057b890(" analysis/SWBFRNT2.ELF-decomp.c      # 64545 64896(lerp) 65312 71994 76952
sed -n 64744,64752p analysis/SWBFRNT2.ELF-decomp.c; sed -n 64890,64900p analysis/SWBFRNT2.ELF-decomp.c
# one projection, far 10000
grep -n "FUN_00580728(" analysis/SWBFRNT2.ELF-decomp.c; sed -n 715603,715660p analysis/SWBFRNT2.ELF-decomp.c
python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF 60c288 4   # 10000.0
# pass ranges → cull only
sed -n 746672,746760p analysis/SWBFRNT2.ELF-decomp.c; sed -n 745226,745262p analysis/SWBFRNT2.ELF-decomp.c
sed -n 746620,746660p analysis/SWBFRNT2.ELF-decomp.c        # FUN_005b0330 near·S < d-2r
grep -n "DAT_007ec4f0\|DAT_007ec4f4\|0x7ec470" analysis/SWBFRNT2.ELF-decomp.c
# Z mapping + CLUT
sed -n 721340,721410p analysis/SWBFRNT2.ELF-decomp.c; sed -n 717729,717770p analysis/SWBFRNT2.ELF-decomp.c
# targets, buckets, frame order
sed -n 720802,720900p analysis/SWBFRNT2.ELF-decomp.c        # FUN_00589fa8
sed -n 719290,719440p analysis/SWBFRNT2.ELF-decomp.c        # FUN_00587278 named node, far clear, viewport
sed -n 719860,719960p analysis/SWBFRNT2.ELF-decomp.c        # FUN_00588968 composite (ALPHA 0x54, TEST 0x33001)
sed -n 721041,721200p analysis/SWBFRNT2.ELF-decomp.c        # FUN_0058a598 kick order
sed -n 698515,698561p analysis/SWBFRNT2.ELF-decomp.c        # FUN_00564638: FUN_00582248 before the tick loop
sed -n 722590,722720p analysis/SWBFRNT2.ELF-decomp.c        # descriptors (FRAME/ZBUF words)
for a in 60c230 60c250 60c2c7 60c2a8 60c4e8 746028; do python3 scripts/tools/elfrd.py extracted/bf2/SWBFRNT2.ELF $a 4; done
# sky TEST selection (decomp + bytes)
sed -n 723411,723480p analysis/SWBFRNT2.ELF-decomp.c
JAVA_HOME=tools/jdk-21.0.12.1+1 tools/ghidra_12.1.3_PUBLIC/support/analyzeHeadless ghidra-proj bf2 -process -noanalysis \
  -readOnly -scriptPath engine/artifacts/lead_tmp/tools/re_sky -postScript DisasAt.java \
  engine/artifacts/lead_tmp/s12/re/skyproj_scratch/disas_58efa0.txt 58efa0 58f060
# BF1
sed -n 554177,554200p analysis/SWBFRONT.ELF-decomp.c; grep -n "FUN_00491048(" analysis/SWBFRONT.ELF-decomp.c
sed -n 587412,587470p analysis/SWBFRONT.ELF-decomp.c; sed -n 572588,572600p analysis/SWBFRONT.ELF-decomp.c
sed -n 567043,567110p analysis/SWBFRONT.ELF-decomp.c
python3 scripts/tools/elfrd.py extracted/bf1/SWBFRONT.ELF 522ac0 4
# GS word decoder used above (TEST/ZBUF/FRAME/TEX0/ALPHA) and Z table: see skyproj_scratch/ (python one-liners in this file's §2.1)
```

## Not closed

- **Far-target pixel height in single view.** The split-screen binding uses a 224-row scissor (720837) while the
  named node in the shipped mode sets a 512×448 viewport (719426) and the composite reads a 512×512 CT16S texture
  (719910–719935); the `DAT_0060c230 == 0` (unshipped) branch is 256×224 (0x7c4f90 descriptors, TEX0 0x620013000,
  719421). Whether the shipped single-view far scene is full height or field height was not resolved. [H: full 512×448]
- The VU1 microprogram (Z bias, clipping, the meaning of packet bits 0x800800 / 0x4000 at 723484–723492) was not
  disassembled; the EE-side header and projection are all that is cited.
- `FUN_00581a98` (716283) emits a second full-screen pass after the colour-only clear in the shipped mode (TEST 0x31001,
  colours `DAT_0060c340/334/32c`) — purpose unknown (fog/haze fill of the far target? [H]).
- `DAT_0060c2a8` (`FUN_00589e38(0)` around the sky tick) → 727615 bucket routing: meaning not established.
- The sky_ key 0xc15d0860 (→ `DAT_00746028`, "scale the far split by zoom") has no preimage found.
- BF1's frame order (named node before the tick, composite in the far-translucent bucket) is asserted by analogy; only the
  binding, clear, TEST rule and composite sites were read.
- The link from weapon `ZoomMin/ZoomMax` to the camera controller's zoom vcall was not walked.
