# LEAD VERIFICATION (Fable lead, session 14, 2026-09-08): VERIFIED.
Re-read by the lead: the PTCH decode re-run (`hoth_ptch.py`; the lead's output is byte-identical — holes at BF1 cp3,
BF2 cp3, BF2 cp4; BF1 cp4 covered 1.72 m below the floor; munge-rule counts 0/357, 0/280, 0/468, 0/160, 0/356,
0/281); the session-13 cutter census re-run byte-identical (the lead's own session-13 correction — the manifest
sphere is radius-first — stands). NOT re-read by the lead this session: the loader/builder lines (546287–546333,
548676–548710; BF2 703562–703628, 706065–706070) and the point test (538028–538057) — verified 18/18 by the
session-14 Fable verifier. Decision: build the replacement mesh (brief_terrain_patch.md; queue item H).
Verification of `lead_tmp/s13/re/hoth_terrain.md` (Fable RE agent, session 13, wrapped early): every cited decomp
line re-read, the raw cutter bytes re-read on the disc, the s13 32-terrain census re-run byte-identical, and §6.1/§6.2
CLOSED by decoding the replacement meshes under both rooms on both discs (`hoth_ptch.py` → `hoth_ptch.txt`, this dir).
No claim in the s13 text was found wrong; two are sharpened: the "+4.8 m crest" cell sits just OUTSIDE the room
prism (the in-room maximum is +6.8/+7.9 m above the floors), and the builder DOES read the HEXP record (shift/offset)
— only never its cutterMask byte. Tags below are mine.

# RE: terrain cutters and the pre-clipped `PTCH` replacement mesh — why the snow rises inside Hoth's outpost rooms (HOLD 19(a))

Corpora: `analysis/SWBFRONT.ELF-decomp.c` (BF1, "B1:"), `analysis/SWBFRNT2.ELF-decomp.c` (BF2, "B2:"); disc offsets
are file offsets in `extracted/{bf1,bf2}/lvl/DATA/_LVL_PS2/HOT/HOT1.LVL`; baked data `engine/assets_baked/{bf1,bf2}_hot1/`.
Tags: [V] read at the line / measured on the disc, [D] derived, [H] hypothesis.

## §0 Headline
1. **The sim is right, the render is wrong.** `CUTR INFO` = `u32 planeCount` + `f32[4] = (r, cx, cy, cz)` — radius
   FIRST (B1:536883–536909: the first float goes to the vector's `.w` via `FUN_0019a030` and to +0x20; B2:694602–694660
   identical). Disc: BF1 HOT1 cutter 14 @0x675F4 `06000000 b1a2de40 608aa5c3 9af7c73f 415e52c2` = 6 planes,
   (6.9574, −331.0811, 1.5622, −52.5920); cutter 15 @0x67680 = 15, (10.7207, −327.5415, 1.5055, −52.3582); `CUTS INFO`
   @0x66D38 = 44. `mungefs/src/terrain.rs:319–323` reads `[g(4), g(8), g(12), g(16)]` and `core/src/collision.rs:673`
   uses `sphere[0]` as r — both correct; no cutter on any of the 32 terrains has r ≤ 0. [V]
2. **Both outpost rooms sit inside two cutters each, on both discs.** Post plane test (`FUN_00474bb0` rule, inside ⇔
   every `n·p + d ≤ 0`): cp3 (−213.01, −4.70, 192.01) inside cutter 41 (max −0.51), on the face of 40 (+0.02); cp4
   (−325.97, −0.84, −52.21) inside 14 (−0.19) and 15 (−0.47); BF2 cutters 12/11 and 3/2 the same to 2 dp. The
   `is_cut_by` sim path already cuts there. [V]
3. **The PS2 render never tests cutters. Each 8×8-cell render patch that a cutter touches ships a PRE-CLIPPED
   replacement mesh in its `PTCH` record** (`INFO` u32 flag + `STRP/POSI/COLR/ALPH`), and the builder draws that
   instead of the procedural grid (§1). The munge clips the patch by the cutter's 3-D VOLUME: over the four rooms,
   0 surviving surface points lie inside a cutter and 0 holes lie outside every cutter (§2). `bake/src/terrain_glb.rs`
   emits the full n×n grid (lines 180–190) and never reads `PTCH` or `cutters` — that is the defect. [V]
4. **The replacement mesh removes the crest inside the rooms (§2, numbers).** At the post the surface is a HOLE on
   BF1 cp3, BF2 cp3 and BF2 cp4; on BF1 cp4 the post's XZ is still covered, at −2.56 m = 1.72 m BELOW the −0.84 m
   floor (the cutter's floor cap is y ≥ −1.31; the terrain there lies under the volume). Over each room's XZ footprint
   the heightfield reaches +6.8 m (cp3) / +7.9 m (BF1 cp4) / +7.2 m (BF2 cp4) above the floor; after the replacement
   the only terrain still above floor+1.8 m is a rim 0.02–2.2 m wide along the west wall (heights −0.6..2.1 m at
   cp3, 3.2..7.1 m at cp4) — terrain outside the cutter's sloped roof, behind the bunker wall. [V]
5. Footprint: 23 flagged patches on BF1 HOT1, 30 on BF2 HOT1; 13 of 32 terrains carry any (BF1 BES2 3, END1 1, HOT1 23,
   NAB2 3, RHN1 5, TAT1 5, YAV2 8; BF2 DAG1 6, END1 1, GEO1 16, HOT1 30, NAB2 10, POL1 11; s13 `census.txt`, re-run
   identical). The flag ⇔ POSI present on all 32; every flagged patch is touched by a cutter sphere. [V]
6. The far sheet `LOWR` is NOT cut at the rooms (it covers both posts, at −8.3/−6.4 m BF1 and −8.2/−2.6 m BF2 — below
   the floors) and is drawn only past `NearSceneRange[1]·zoom`, so it is invisible inside the rooms. [V]

## §1 The mechanism on the ELF
1.1 **Loader** (BF1 `FUN_00481608` B1:545920; `PTCH` 0x48435450 at 546272; BF2 `FUN_0056bbf0` 703226, PTCH 703562).
Per PTCH a 0x18-B record (BF2 0x14). `INFO` (B1:546287–546305): u16 → +8 (layer mask), u16 → +0xa (count, prefix-summed
into +0xc), u16 → +0xe (heightMin), u16 → +0x10 (heightMax), u16 → +0x12 (shift), then `u32 flag`: `if (flag != 0)
side = alloc(0x18); rec+0x14 = side` (546302–546306; BF2 703583–703590, rec+0x10). Sub-chunks: `POSI` 0x49534f50
→ `side[0] = size/6`, bytes at `side+0xc` (546314–546320); `STRP` 0x50525453 → `side[1] = size>>1`, at `side+8`
(546322–546326); `COLR` 0x524c4f43 → `side+0x14` (546327–546330); the *(decompiler detail omitted)* tag → `side+0x10` (546310–546313),
which BF2 names `ALPH` 0x48504c41 (703596–703602). BF2 POSI 703604–703611, STRP 703613–703621. [V]
Disc `INFO` (14 B) = `u16 mask, u16 count, i16 min, i16 max, u16 shift, u32 flag`: BF1 PTCH 588 @0xA267C = (7, 0, −35, 43,
9, 1) with STRP 470 / POSI 738 / COLR 3 / ALPH 106 B; PTCH 490 @0x9D794 = (7, 0, −36, 79, 9, 1); BF2 588 @0xAD4B4 =
(7, 0, −35, 42, 9, 1), 490 @0xA8A8C = (7, 0, −36, 79, 9, 1). [V]
1.2 **Builder** (BF1 `FUN_00485f20` B1:548247; BF2 `FUN_00570040` 705577). Vertex budget 548317: `rec+0x14 == 0 ?
(N²+N)·0x10 : side[1]`; 548331 `if (side == 0)` → the procedural 8×8 packer (548332–548675; it reads the HEXP record
`DAT_005647d8` for shift/offset at 548357 etc. and HGT8 `DAT_005647dc` at 548377 — but never the `+1` cutterMask
byte: no `DAT_005647d8 + 1` read in 548247–548712). ELSE 548676–548700:
```
posi = side+0xc; strp = side+8;  per index: v = posi[(idx & 0x7fff)*6]                     // 548684–548688
ux·N, uz·N (N = DAT_0056485c = 8, 536767)                                                   // 548689–548690
VU word = raw 6 B | (bit15 ? 0xc000<<48 : 0x4000<<48)                                       // 548691–548695
uv = (ux·N·0x800/0xffff − N/2·0x800) | (… uz …) << 16                                        // 548696–548697
```
VU box 548703–548710: min `(px·gu − half, heightMin·hs, pz·gu − (half−gu))`, max `(x0 + DAT_00522a18,
(heightMin + (0x10000 >> shift))·hs, z0 + DAT_00522a18)`, `DAT_00522a18 = gu·N·1.0000153` (546004) = 65536/65535·64 m.
BF2: 705673 `+0x10 == 0`, 706065 `& 0x7fff) * 6`, 706068–706070 the 0xc000/0x4000 words. [V]
So POSI = `u16 x, y, z`: `X = x0 + x/65536·64.001`, `Z = z0 + z/65536·64.001`,
**`Y = (heightMin + y / 2^shift) · heightScale`** — the FRACTIONAL form. Proof on the disc: every replacement vertex
that sits on a cell corner (x, z ≡ 0 mod 0x2000) reproduces the HGT8/HEXP height to < 5 mm — 58/58, 64/64, 64/64
(BF1 588/458/490), 63/63, 64/64, 62/62 (BF2) with worst error 0.000 m (`hoth_ptch.txt`). Closes s13 §6.2. [V]
STRP: a strip restarts at a flagged PAIR (`mungefs::model::unstitch_strips`, model.rs:349–386); alternate winding
gives 150–168 up-facing triangles per patch and, on BF1 PTCH 588 only, 18 FLAT triangles wound the other way (unit
normal.y = −1.000) — a parity slip of the pair heuristic on a few strips, not overhangs. The exact restart rule stays
open (§5); the port draws the world with `cull_mode: None` (render/src/lib.rs:1475), so winding does not matter.
1.3 **HEXP `cutterMask`** (byte 1 of the 8-B record) is consulted only by the sim's point test (§1.4), never by the
builder (above). Flag set ≠ mask set: BF1 HOT1 flag−hexp = {621}, hexp−flag = {459, 495, 558} (s13, census re-run). [V]
1.4 **Cutters on the ELF.** Loader `FUN_004726b8` B1:536568: `CUTS` 0x53545543 (536853) `INFO` u32 count →
`…
`CUTR` 0x52545543 (536867): `INFO` = u32 planeCount → +0x30, then f32 r → `.w` and +0x20, f32 cx cy cz → +0x10
(536883–536909); `PLNS` 0x534e4c50 → planeCount × f32[4] (536927–536937). Point test `FUN_00474d98` B1:537988:
HEXP byte-1 quadrant gate 538028–538034 (`bit = (x>>3)&1 | (z>>2)&2`), then per cutter `FUN_00474c20(r, p, c)` =
`|p − c|² < r²` (537901–537921) and `…
height wrapper `FUN_00473db0` returns −300.0 when cut (537211–537228); BF2 `FUN_0055f258` 695207–695225. [V]
Port deltas: `collision.rs:679` tests `< 0.0` (ELF: `≤ 0`), and the port has no HEXP quadrant gate — both edge-only.

## §2 The four rooms, decoded (`hoth_ptch.py`; floor = the post's y; samples on a 0.5 m grid inside the room prism's XZ)
| room | PTCH under the room (flag) | mesh verts / tris / XZ hole | heightfield in the room: max − floor | PS2 surface: holes / covered / above floor+1.8 | at the post |
|---|---|---|---|---|---|
| BF1 cp3 | 588 (1) | 123 / 168 / 7.5 % | +6.77 m (176/637 samples above eye) | 280 / 357 / 47 (rim ≤ 1.65 m from the west wall) | HOLE (heightfield −4.45, floor −4.70) |
| BF1 cp4 | 458, 490 (1, 1) | 89+101 / 136+148 / 0.2+0.8 % | +7.92 m (130/628) | 160 / 468 / 47 (rim ≤ 1.57 m) | covered at −2.56 (floor −0.84) |
| BF2 cp3 | 588 (1) | 103 / 151 / 1.7 % | +6.78 m (176/637) | 281 / 356 / 47 (rim ≤ 1.65 m) | HOLE (−4.45 / −4.70) |
| BF2 cp4 | 458, 490 (1, 1) | 89+98 / 136+145 / 0.4+1.6 % | +7.21 m (218/628) | 327 / 301 / 71 (rim ≤ 2.20 m) | HOLE (−0.16 / −0.84) |
Munge rule: covered samples whose surface point lies inside a cutter volume 0/357, 0/468, 0/356, 0/301; hole samples
whose heightfield point lies outside every cutter 0/280, 0/160, 0/281, 0/327 → **the munge removes exactly the
terrain inside the cutter's 3-D volume** (the ASCII maps in `hoth_ptch.txt` show the rounded room as the hole). The
baked `terrain_hoth.glb` equals the HGT8/HEXP reconstruction to 4e-6 m on both discs (so the bake's grid is right;
only the un-cut patches are wrong). The s13 "+4.8 m" cell (100,151) is 1 m outside the prism's west face. [V]

## §3 What sits there (manifest `instances`, both discs) [V]
BF1: `hoth_bldg_bunker` bunker1 (−213.991, −2.726, 192.0) identity; bunker2 (−326.737, 1.179, −52.305) yaw
(0.99782, 0.066); `com_bldg_controlzone` cp3 (−213.014, −4.699, 192.011), cp4 (−325.971, −0.837, −52.213). BF2:
bunkers (−214, −2.726, 192) and (−326, 1, −52); the same posts. Cutter 41 (BF1) / 12 (BF2) centre (−214.80, −2.40,
192.00) = bunker1 + (−0.81, +0.33, 0); BF2 cutter 3 (−326.80, 1.33, −52.05) = bunker2 + (−0.80, +0.33, −0.05): the
bunker model's two cutter volumes (a 6-plane box = the entrance, a 15-plane prism = the round room, roof y ≤ +0.41 at
cp3) placed by the instance [D]. HOT1.LVL holds 44 `CUTR` (+ `CUTS`) and no other "cut" string.

## §4 Port rule (bake + render; the sim already cuts) — PROVISIONAL until goldens
```
mungefs::terrain: per PTCH parse INFO (u16 mask, u16 count, i16 min, i16 max, u16 shift, u32 flag); when flag != 0
    parse POSI (n × u16 x,y,z) and STRP (m × u16); keep COLR/ALPH aside (not decoded).
bake::terrain_glb: for a flagged patch (px, pz) DROP its 64 procedural quads and EMIT the replacement triangles:
    X = x0 + x/65536·(8·gu·65536/65535), Z = z0 + z/65536·(…), Y = (min + y/2^shift)·hs,
    x0 = px·8·gu − half, z0 = pz·8·gu − (half − gu); strips via mungefs::model::unstitch_strips;
    normals / UV / LUMI for the new vertices by sampling the heightfield at (X, Z) (they lie on it: §1.2).
render: nothing new — different triangles in the same GLB; the world pass already draws without culling.
sim: unchanged (is_cut_by is right; optionally `<=` and the HEXP quadrant gate — edge-only).
```
Footprint: 13 terrains, 1–30 patches each (§0.5); POSI 564–19 806 B per map. Expected on the walkabout: the snow
mound inside both Hoth rooms gone on both discs (panels in `walkabout_defects/README.md` group 2); a ≤ 2 m rim of
terrain may remain visible at the west wall's foot where the cutter roof slopes — the PS2 has it too.

## §5 Not closed
- The exact STRP restart rule (the pair heuristic flips 18 flat triangles on BF1 PTCH 588); moot for drawing.
- `COLR` (3–6 B) and `ALPH` (33–117 B) per flagged patch — not decoded (tint/layer weights of the new vertices).
- BF2's point test `FUN_0055fdd0` and the ≥ 11-cutter lookup (`DAT_005229ac`) — not read.
- Whether the VU's sphere sum includes `.w` (no effect on the render).

## §6 Reproduction
*(decompiler excerpt omitted from the public copy)*
