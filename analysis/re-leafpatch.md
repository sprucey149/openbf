# OPUS RE (2026-09-29) - [O] UNAUDITED
# RE: leaf patches — the sprite clusters the PS2 hangs on trees (card LEAF)

> **AUDIT 2026-09-30 (loop lane 1, PARTLY — `engine/artifacts/loop/audits.md`, report
> `engine/artifacts/loop/reports/20260930_0330_audit_LEAF.md`).**
> - **Method.** Re-derived before reading this doc from the B1/B2 decomp and ELFs, the disc's classes, PS2 RAM in all
>   96 library states of the 9 leaf maps (own readers), 13 patched-state PCSX2 boots, and the port (main 515b024,
>   scratch examples). Evidence: `engine/artifacts/lead_tmp/audit-leaf/`. One Opus verifier: 13 SUPPORTED, 3 PARTLY
>   (folded in here), 0 NOT.
> - **Held.**
>   - **Placement.** The attach rule, its persistent global (ELF stores: 2 per global, all in `FUN_001fb6e8`), caps
>     30/16, up + origin frames and MaxDistance all hold. Every one of 35,448 live objects in the 96 states is one the
>     port's `objects()` makes: origin ≤ 8.7e-5 m, axes ≤ 3.6e-7, no extras. All 968 live FOLG `LeafPatch` instances
>     in the 35 YAV states match the port's (≤ 1.7e-4 m, colour, distance, class).
>   - **Templates.** Given each state's own seed, the port's `template` reproduces every live template in the 96
>     states, both BF2 loads included (261 comparisons: ≤ 9.5e-6 m, greys ≤ 1.9e-5).
>   - **Draw.** Fades (§6.1), the per-view buffer and quad, the UV tables (ELF bytes, both games), the sway (the RAM
>     table is sin) and the GS words (RAM, 7 states) all hold.
>   - **The §6.1 [H] is settled on the PS2.** With the leaf TEST patched to never, BF2 KAS2 frames lose nothing where
>     its leaf objects sit at 161–463 m (0.5–1.8 % of mask pixels change by > 16, against 17.6 % in a near control
>     at 35–111 m). The far pass does not draw them, so the port's NearSceneRange cut is right. The 100–161 m band is
>     untested.
>   - **Colour.** The leaf-pixel mean gap to the PS2 at 6 views is 1.2–4.7 levels (BF2) and 9.4 / 10.7 (BF1, the port
>     ~10 short in blue), at threshold 16.
> - **Wrong: "unseeded classes take a random seed per session — any seed is as faithful" (§0, §4.2; the port's
>   `leaf.rs` doc).**
>   - **The reseed.** The level load resets the global LCG to 0: B1 `FUN_00173a98` 16151, B2 `FUN_00174b28`
>     16762/16876. An ELF scan found no other non-stepping store, except the boot constant 0x94153a94.
>   - **Seeds are fixed draws.** Every unseeded seed is the sequence's state after n draws.
>     - BF1: the same n in every state of two independent loads. YAV1: 1648, 1650, 13112, 13114. YAV2: 1626, 1628,
>       1630, 1632.
>     - BF2: two loads that differ in era and AI count give different n (DAG1 3154 / 3284, KAS2 3720 / 4002, YAV1
>       6862… / 6930…). On COR1, two GCW loads with 33 and 0 soldiers share their draws, so the era is the likely
>       driver [D: one map].
>   - **Tints.** Object tints are draws of the same sequence (all 530 of BF1 NAB1's at steps 1654..8360), fixed per
>     BF1 map.
>   - **Visible cost.** With the PS2's seeds, the port's leaf pixels overlap the PS2's better on BF2 DAG1 (IoU 0.38 →
>     0.58) and a little on KAS2 (0.45 → 0.48). BF2 YAV1 moves only marginally; BF1 YAV1 does not improve.
>   - **Card LEAFS** (value 4).
> - **Side [D].** The port's `fs_grass` fogs leaves while the PS2's leaf draw binds no fog pass. This touches only the
>   two YAV1 maps (fog from 90 m, ≤ ~8 % at the NearSceneRange end); no card.

Card **LEAF** (M1), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/re/leaf/` — `census.py`
(every `leafpatch` class and every `AttachOdf` user on both discs → `props.txt`, `attach.txt`), `ram.py` (the live
classes, sprite templates and objects in a PCSX2 save state), `template.py` (a transcription of the template builder,
compared with RAM → `template_check.txt`), `attach_check.py` / `verify/attach_persist.py` (every attached object
predicted from the port's manifest × the model skeleton → `verify/attach_persist_all.txt`), `skel.py` (bind matrices,
the port's convention), `leafmask.py` (where the leaf sprites fall on a state's frame → `masks/`), `ps2exp/` (patched
save states + `measure.py` → `measure.txt`, `frames/sheet_*.png`), `claims.md`, `checks.sh` → `checks.txt` (every §5
self-check, verbatim), `verify/VERDICTS.md`. Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 =
`analysis/SWBFRNT2.ELF-decomp.c`. Tags: **[V]** read at the cited lines and re-checked; **[D]** derived; **[H]**
hypothesis; **[O]** load-bearing for the build; "RAM" = read in the PS2's own memory from a save state; "PS2" = a
PCSX2 frame.

## 0. TL;DR

- **A leaf patch is a sprite cluster class** (`leafpatch`, factory B1 `FUN_001b21e0`, B2 `FUN_001ca9d8`) [V]. It gets
  into the world four ways [V][O]: attached to a prop's skeleton hardpoints (`AttachOdf` + `AttachToHardpoint` in the
  prop's ODF — nearly every leafy tree), placed as a world instance (BF1/BF2 NAB2's 40 vines, BF2 COR1's 37 planter
  patches, BF1 YAV2's 3 bushes), attached to path-placed props (BF2 DAG1's 31 hellbushes, FOLG §9), and as a FOLG
  `LeafPatch` layer (BF1 YAV1/YAV2 and BF2 YAV1 bushes; FOLG places those already).
- **The attach rule** [V][O]: `AttachOdf` sets a *current class* (a global); each `AttachToHardpoint` appends
  (current class, the model's hardpoint matrix) — up to 30 (BF1) / 16 (BF2) per class, only hardpoints that exist in
  the model. The global is never reset between classes, so hardpoints listed before a class's first `AttachOdf`
  inherit the previous class's (DAG1 authors four such classes). RAM: all 3,078 live objects in 9 states are
  accounted for — 2,958 predicted as attachments from the port's manifest + the disc skeletons, 120 world instances (§2).
- **One sprite template per class** [V][O], built by B1 `FUN_001b38f8` / B2 `FUN_001cbf10` from the class `Seed`:
  NumParticles points in an upper half-ellipsoid (Radius × Height), a box, or a recursive branching vine; sizes in
  [MinSize, MaxSize); a grey per sprite shaded by the sun direction; a random sway phase; an atlas part `i % 4`.
  **`template.py` reproduces all 23 live templates in 9 states of both games** (§4). Unseeded classes (the YAV, DAG1,
  KAS2 ones) take a random seed per session.
- **The draw** [V][O]: each object is the class template under its own matrix (up from its hardpoint, yaw dropped,
  up axis × the class's vertical scale), drawn in the sorted translucent pass; per view the template's sprites are
  sorted back to front and become camera-facing quads (a per-sprite ±0.3·(i % 5) twist, the top edge swayed by the
  wind); texture × grey × a per-object tint in [0.7, 1]; faded by MaxDistance (last 40 %), by NearSceneRange b..c and
  near the camera (< 6 m).
- **GS state, proven on the PS2** [V measured]: blended (ALPHA 0x44), alpha test ≥ 64/128 writes Z, texels below it
  are still blended into colour without Z (TEST 0x5140b, AFAIL = FB_ONLY). Patching the TEST to never-pass removes the
  canopy and nothing else (|Δ| outside the leaves 0.00–0.06); on BF2 YAV1 most of the visible canopy is the blended
  below-threshold part, so **the port must blend, not alpha-test**.
- **BF1 leaves are unlit; BF2 leaves are lit** [V measured]: with the scene lights zeroed, BF1 NAB1's leaves keep
  their colour (−3 levels while the scene drops 21) and BF2 DAG1's lose ~70 % (per-vertex normals ±Y and a light-set
  upload, §6.6).
- **At the 2026-09-30 research baseline, the port drew none of it** [V]: 0 hits for `AttachOdf` / `AttachToHardpoint` / their hashes in `engine/crates`;
  FOLG places `LeafPatch` records that nothing draws; hardpoints are baked only for vehicles and armed buildings.
- **Not leaf patches** [V measured]: BF1 NAB1's round crowns past ~120 m are the tree's far model
  (`nab_prop_trees2LOWD`, a leafy crown billboard) — the far scene draws LOWD (`re-far-lowd.md`), the port draws LOD0
  (bare branch cards) → card **LOD**. Falling leaves (the `LeafSpawner`) are a separate particle effect (§8).

## 1. The disc [V] (`census.py`, `props.txt`, `attach.txt`; world vs path counts from the port's manifests)

| map | leaf classes (NumParticles, size m, Radius × Height, S = 0xcaaa3778, MaxDistance, texture, NumParts, Seed) | sources → objects |
|---|---|---|
| B1 NAB1 | `nab_prop_leafpatch` (60, 5.7–6.0, 9.5×8, 1.0, 120, nab_leaf, 1, 321651321) | 106 `nab_prop_trees2` × hp_branch_1..5 → 530 |
| B1 NAB2 | `nab2_prop_leafpatch` (55, 1.7–2.0, 4×4, 1.0, 80, nab2_leaf, 1, seeded), `_tall` (50, 1.2–1.4, 1.3×13, 1.6, taper 8, 80), `nab2_prop_vine1`/`2` (50, vine mode (2, 0) / (0, 0.9), 80) | 65 leaftree + 55 leaftree_tall × hp_branch_01 → 120; 26 + 14 world vines |
| B1 YAV1 | `yav_prop_leafpatch` (36, 6.0–6.5, 11×1.5, 0.5, 120, yav_leaf, 1, unseeded), `…2` (32, 4.0–4.5, 9×2, 0.5), `yav_prop_bush`/`bush2` (50/40, ~1.1–1.5, 2×1.5/1, offset y 0.4/0.3, 30) | leaftree 64×9, leaftree2 30×6, leaftree3 24×3 → 828; FOLG layers 2–3 (bush2 35 m, bush 40 m) |
| B1 YAV2 | as YAV1 | leaftree2 32×6 → 192; 1 + 2 world bushes; FOLG layers 2–3 |
| B2 YAV1 | `yav_prop_leafpatch` (30, 7–8, 12×1, 0.5, 1000, yav_leaf, 4), `…2` (20, 5–6, 10×2, 0.5, 1000), bushes | 828 as B1; FOLG layers 2–3 (the same `prp_`: FOLG §8 lists B1 only) |
| B2 DAG1 | `dag1_prop_leafpatch` (30, 7–8, 12×1, 0.5, 1000, Dag1_vine, 4, unseeded) | grandtree 21×3, barrier_1 13×6, grandtree_3 3×1 (both inherited, §2.2), treegroup_1 3×9, treegroup_2 2×12, vine_02 2×1, 31 path hellbushes ×3 → 290 |
| B2 KAS2 | `kas_prop_leafpatch` (30, 10–15, 10×4, 0.6, 1000, kas_leaf, 4, unseeded) | 4 treegroup × 12 found (7 of 19 listed hardpoints are not in the model) → 48 |
| B2 NAB2 | as B1 NAB2 (MaxDistance 1000 on the leaf classes, 80 on the vines) | 65 + 57 → 122; 26 + 14 world vines |
| B2 COR1 | `Cor1_prop_leafpatch_tall` (as nab2 `_tall`, MaxDistance 1000, Cor1_leaf) | 37 world instances; `cor1_bldg_walk_young`'s hp_plant* are not in its skeleton → 0 attached |

Every leaf class is `BASE leafpatch`. The YAV `leafpatch`/`leafpatch2`, DAG1 and KAS2 classes set the falling-leaf
count `0xfd7fae90` = 6; those and the YAV bushes set the sound byte `0x3b1948f3` = 3 (§8); NAB and COR1 set both 0.
NAB1's class also sets the wind (0.5, 0.09). A class's copies in the `_H` / sibling LVLs are identical.

## 2. How a leaf patch gets into the world

### 2.1 The prop's attach list [V][O]
- B1 prop/building class setter `FUN_001fb6e8` (the `prop` and `building` factories are `FUN_001fb390`, 15553–15557;
  its attach fields +0x114/+0x118 read back from the live `nab_prop_trees2` class in RAM):
  - `AttachOdf` 0xa9d0d48b (109410–109423): hash the name (FNV `FUN_00444b88`), walk the class list `DAT_005842b4`
    (node+0xc = class, class+0x14 = name hash); found → `DAT_00517848` = class, `DAT_0051784c` = 0; else both 0.
  - `AttachToHardpoint` 0x3be7b80a (109163–109189): with `DAT_00517848` ≠ 0: return when the count +0x118 is 30
    (0x1e); allocate 30 × 0x50 on the first; entry +0x40 = the class, +0x44 = −1.0; parse `"%s %f"` (0x532350) into
    the hardpoint name and +0x44; `FUN_00496f18(model+0x1c, CRC32(name), entry)` copies the model's hardpoint matrix
    (table *(model+0xb0): matrices, hashes, count; B1 557826–557863) and the count grows only if found.
  - `AttachEffect` 0x6a6c7e0d (109296–109300) sets `DAT_0051784c` (an effect) and zeroes `DAT_00517848`; its
    hardpoints go to a second list (+0x11c/+0x120).
- Instance creation `FUN_001fc860` (109619–109760): per entry, world = entry matrix × instance matrix (row-vector),
  and the attached class's create-instance (vtable +0x1c slot +0x10) builds the child there; `+0x44 > −0.0001`
  overrides the child's `MaxDistance` (109715) — no leaf ODF uses the float.
- B2 `FUN_0022f6e8`: the same with `DAT_005fd02c`, cap **16** (0x500 bytes; `0xf < count` returns, 145957),
  `FUN_0057dc60` for the matrix (145982). `AttachOdf` (146305–146326) zeroes the global, looks the class up in
  `DAT_00688e84`; not found → returns with 0; found and its vtable(+0x1c)+0x14(class, `DAT_00689578`) = 0 → the global
  = class; found and nonzero → handed to the base setter `FUN_00214600` (the global stays 0). Leaf classes take the
  first path (RAM, §2.3). `AttachToHardpoint` with the global 0 → `FUN_00214600` (145954).

### 2.2 The global is not reset between classes [V][O]
`DAT_00517848` / `DAT_0051784c` are written only by those two branches (writers + `addrscan`/`widescan`: stores
0x1fc2c8, 0x1fc2d0, 0x1fc300, 0x1fc308 — all in `FUN_001fb6e8`); the `GeometryName` branch (0x47c86b4a, 109217)
resets `DAT_00517830`–`DAT_00517844` (109241–109246) but not them. B2 `DAT_005fd02c`: 3 stores, all in
`FUN_0022f6e8` (145997 AttachEffect, 146306/146323 AttachOdf). So a class's `AttachToHardpoint` lines that precede
its `AttachOdf` attach the class current from the previously loaded class. On the discs only BF2 DAG1 does this
(`dag1_prop_grandtree_2` H H H O, `barrier_1` H×8 O, `grandtree_3` H H O, `grandtree_4` H O H…); in DAG1.LVL's class
order each follows a class ending on `dag1_prop_leafpatch`, so `barrier_1` and `grandtree_3` get leaves, and
`grandtree_2` (loaded first) gets none — its three hardpoints are not in its model anyway.
**Bake rule** [O]: walk the map LVL's `entc` chunks in file order with one current class (AttachEffect clears it; a
name the map does not define clears it), cap 30 / 16, keep entries whose hardpoint (CRC-32 of the lower-case name =
the port's `SkelBone::crc`) exists in the model's skeleton, store the bind matrix.

### 2.3 RAM check [V measured] (`verify/attach_persist.py`, `attach_persist_all.txt`)
The rule above, run on the port's manifests (world instances and `path_props`) and the disc skeletons (`skel.py`:
M0[i] = L[i] × M0[parent], `mungefs::skel::bind_matrices`), predicts every live attached object:

| state | live leaf objects | predicted & matched (≤ 1 cm) | the rest |
|---|---|---|---|
| B1 NAB1 cp1_cp2 | 530 | 530 (worst 8.5e-5 m) | — |
| B1 NAB2 rotunda | 160 | 120 | 40 world vines |
| B1 YAV1 bazaar | 828 | 828 | — |
| B1 YAV2 cp0_cp2 | 195 | 192 | 3 world bushes |
| B2 YAV1 landing zone | 828 | 828 | — |
| B2 KAS2 cp0 | 48 | 48 | — |
| B2 NAB2 cp4_cp6 | 162 | 122 | 40 world vines |
| B2 DAG1 cp4_cp3 | 290 | 290 (209 without the inherited class) | — |
| B2 COR1 cp1_cp2 | 37 | 0 | 37 world instances |

World instances sit at their instance's position with its row 1 as up (≤ 1.1e-5 m). NAB1's RAM hardpoint table equals
`skel.py`'s bind matrices exactly (the verifier: 6/6 names by CRC). The PS2 creates DAG1's path hellbushes with their
leaves, and the port's FOLG `path_props` put them within 1 cm of the PS2's.

## 3. The class [V][O] (setter B1 `FUN_001b27b8` 57475–57643, ctor `FUN_001b21e0` 57277–57341; B2 `FUN_001cadc0` /
`FUN_001ca9d8`: the same hashes, fields +0x20 — the bound radius +0x5c → +0x78)

| hash | name | B1 field | default | meaning |
|---|---|---|---|---|
| 0xd133f7da | NumParticles | +0x9c (int) | 50 | sprites; **÷ the local view count** (57572; B2 75503); also the drawn count +0x110 |
| 0xeaebcf6e / 0x80ba1308 | MinSize / MaxSize | +0xb0 / +0xb4 | 10 / 15 | sprite size range (m) |
| 0x0dba4cb3 / 0xd5bdbb42 | Radius / Height | +0x84 / +0x8c | 5 / 5 | the half-ellipsoid |
| 0xcaaa3778 | (vertical scale "S") | +0x88 | 1.0 | the object's up axis × S at draw; template y stored ÷ S |
| 0x14c8d3ca | Offset | +0xa0..a8 | 0 | added to every sprite (y ÷ S) |
| 0xeaaaa7a9 | (taper) | +0xc0 | 0 | the top c0 metres narrow to (Height − y)/c0 |
| 0x5045bcac | Seed | +0x90 | a draw of the global LCG at class creation (57319) | `%i`; values past 2³² wrap (RAM: 5232344514 → 937377218) |
| 0x35a4393b | (box) | +0xd0..d8, mode +0x94 = 2 | — | uniform in a box instead |
| 0xcdbc383d | (vine) | +0xf4, +0xf8, mode 3 | 1, 0 | the vine's sideways wobble (x, z) |
| 0xa8c0d41d | (vine run) | +0xfc, +0x100 | 2, 4 | straight-run length range |
| 0x45328dc6 | (vine step) | +0x104 | 0.6 | step = size × this |
| 0x13fac25e / 0x25e5c3fc | DarknessMin / Max | +0xe4 / +0xe8 | 1 / 1 | grey range |
| 0x5d8b6dab | Alpha | +0xb8 | 0.5 | sprite alpha |
| 0xe6915cf6 | MaxDistance | +0xbc | 100 | draw distance |
| 0x3c6468f4 | Texture | +0xe0 | "grass" | texture (hash) |
| 0x5ac02207 | NumParts | +0xf0 | 4 | 1 → whole-sheet UV table, else quarters |
| 0xd4cbd35b / 0x7cbb796c | (wind speed / amplitude) | +0x108 / +0x10c | 1.2 / 0.11 | §7 |
| 0x1abd7877 | (draw count) | +0x110, +0x114 = 1/(0.2n) | = NumParticles, 1.0 | a far-first cull + an alpha ramp; no disc class sets it |
| 0xfd7fae90 | (falling leaves) | byte +0x80 (≤ 50) | 0 | §8 |
| 0x3b1948f3 | (sound slot) | byte +0x81 (≤ 5) | 0 | §8 |
| — | near fade | +0xec | 6.0 | no setter |

Names in parentheses are unrecovered hashes (the build keys on the hashes). The disc values are in §1 and `props.txt`.

## 4. The template (one per class) [V][O]

B1 `FUN_001b38f8` (57961–58270) / B2 `FUN_001cbf10` (75891–76173), run once, when the class's first instance is
made (`FUN_001b4428` 58281) or a FOLG layer resolves the class (`FUN_003474c0` 333083; B2 76185, 487526).
`u()` = the next LCG float in [0, 1) (state·0x19660d + 0x3c6ef35f, two steps per float: `((a >> 16) | (b & 0xffff0000))
>> 9 | 0x3f800000` − 1); state₀ = LCG(LCG(Seed ^ 0xc71c2830)) (58029; 75952). For each sprite i:
1. **Position.** Mode 0 (default): repeat {x = u·R·1.5, y = u·R, z = u·R·1.5 (draw order x, y, z by LCG pairs
   1–2, 3–4, 5–6: x from the first pair, y the second, z the third); x = −x when q < 2, z = −z when (q − 1) < 2
   unsigned; q = (q + 1) mod 4 (q counts every try, across sprites)} until x² + y² + z² ≤ R²; then
   y ·= Height/(S·R); if c0 = +0xc0 > 0 and y > Height − c0: x, z ·= (Height − y)/c0. So points fill the upper
   half-ball of R, squashed to Height/S, spread evenly over the four quadrants. Mode 2: ((u − ½)·w, (u − ½)·h,
   (u − ½)·d). Mode 3 (the vine, §4.1): positions and sizes come from the walk.
2. **Size** (modes 0, 2): MinSize + u·(MaxSize − MinSize).
3. **Grey**: Δ = DarkMax − DarkMin, g = u·Δ; n = normalize(position) (a zero vector → n = 0); grey = DarkMin +
   g·½ − (n · sun)·Δ·½, sun = B1 `DAT_0069c590` (`FUN_0049b8a8`, the manifest's `sun_dir` source,
   `re-model-lighting.md` §6) / B2 *(`DAT_0060c6e0`)+0x50; so the side facing the sun is lighter. RGB = grey, A = Alpha.
4. **Phase** = u·100; **part** = i % 4; position += (Offset.x, Offset.y/S, Offset.z).
5. The class bound: min/max over (x ± size/2, (y ± size/2)·S, z ± size/2) from ±100 → centre +0x70, half +0x60; radius
   +0x5c = max over sprites of |(x, y·S, z) − centre| + max(size/2, size·S/2).

### 4.1 The vine (mode 3) [V][O] — B1 `FUN_001b2d38` (57666–57958; `vine_b1.dis`), B2 `FUN_001cb340`
`vine(pos, dir, n)`, called with pos (0,0,0), dir (0,1,0), n = NumParticles (0x1b3a58–0x1b3a88):
k = n − 1 (return if < 0); a sprite at pos, size = (Min + uΔ)·min(1, n/4); run = min(trunc(u·(p100 − pfc) + pfc), k);
while run: {f = min(1, (k + 1)/4); k −= 1; dir = normalize(dir + ((u − u)·v0/2, 0.2, (u − u)·v1/2)); pos += dir·size·
p104; a sprite at pos with size = (Min + uΔ)·f}; if k > 0: a = (u − u)·v0, b = (u − u)·v1, nA = trunc(k·(u/2 + ¼)),
dirA = normalize(dir + 0.8·((a, 0, b) − dir)), vine(pos + dirA·size·p104, dirA, nA); then new a, b, dirB likewise,
vine(pos + dirB·size·p104, dirB, k − nA). Constants: 0.25 (0x3e80), 0.2 (0x3e4ccccd), 0.8 (0x3f4ccccd), 0.5 (0x3f00).

### 4.2 RAM check [V measured] (`template.py` → `template_check.txt`)
All 23 live class templates in 9 states of both games (NAB1, NAB2 ×2 incl. both vines, YAV1 ×2, YAV2, B2 YAV1, DAG1,
KAS2, COR1) reproduce from the transcription: positions ≤ 1e-5 m, sizes ≤ 1e-6, greys ≤ 2e-7, parts and alpha
exact, bounds ≤ 8e-6 (it reads the class fields and the sun from RAM, so the ODF parse is not what it tests). The
unseeded classes' seeds differ between sessions (verifier: `dag1_prop_leafpatch` 3172847945 vs 1072521877), so
their layout is not recoverable from the disc — any seed is as faithful [D].

## 5. The object [V][O]

- B1 `FUN_001b1448` (56743–56953) / B2 `FUN_001c9c38`: class at +0x160 (B2 +0x170); matrix: **up = the given
  matrix's row 1; row 0 = normalize((0,0,−1) × up); row 2 = normalize(row 0 × up); position = row 3** — the yaw
  (and any scale) of the hardpoint / instance is dropped, so every object of a class is the same sprite cloud turned
  only by its up axis. RAM: 2,883 live objects match to 1.2e-7 (1,168 with |up.y| < 0.999).
- Tint (the draw colour): R, G, B = 0.7 + 0.2·u + 0.1·u′ (u shared, u′ per channel, the game's global LCG), A = 1
  (56917–56931; ELF 0x3e4c/0x3dcc/0x3f33). MaxDistance +0x17c = the class's (56943).
- A collision object (+0x50; B1 layer 0x100, B2 0x200) is registered only when the falling-leaf byte ≠ 0 and not a vine
  (56934–56941) — for the leaf spawner's query (§8); nothing in LEAF's scope collides with it.
- **Draw order**: B1 `FUN_001b1f00` (57209–57265) first inserts the object into the sorted translucent heap
  (`FUN_004c67e8` → `FUN_004c6888`, key = depth + radius); the flush `FUN_004c6928` (587551–587603) pops the farthest
  first and calls each object's render, which then draws (§6). So clusters are drawn far to near among the translucents.
- The FOLG `LeafPatch` layers are not objects: B1 `FUN_003465c0` 332700–332733 (B2 `FUN_0042c788` 487064) calls the
  same draw with the `prp_` entry's distance (not MaxDistance), the FOLG instance matrix (FOLG §6/§7) and the FOLG
  tint (the LUMI tint, bytes/255) as the colour.

## 6. The draw [V][O]

### 6.1 Cull and fades (B1 `FUN_002ee970` 277485–277586, B2 `FUN_003c87e0` 423424–423538)
d = cam row 2 · (eye − p) (row 2 is the back axis: d > 0 ahead); s = cam+0x90 (the zoom, 1.0 in play);
D = MaxDistance·s (B2 also × `DAT_005ffc60` = 1.0, never written). Drawn iff −bound ≤ d ≤ D, then
- α = min((D − d)/(0.4·D), 1) — **a fade over the last 40 % of D**;
- d < +0xec (6 m) and not a vine: α = max((d/6)², 0.1) (replaces the above);
- unless draw flag 0x20000: for d > b·s, α ·= 1 − (d − b·s)/((c·s + bound) − b·s), and no draw when ≤ 0 — b, c =
  NearSceneRange (`DAT_00677660`/`664`, `FUN_00479c10`; defaults 72/120). The far-scene flush (`FUN_002f2308` →
  `FUN_004c6608(…, 0x60000)`) sets 0x20000; whether leaf objects reach that pass is [H] (on BF1 YAV1 and BF2 YAV1/
  DAG1/NAB2 `FarSceneRange` is 0: no far scene at all).
- The matrix's row 1 × S (277565) → `FUN_002eeba0`. B2's draw is gated by `DAT_005fc5e8` (1, three loads, no store).

### 6.2 Per view: the sprite buffer (B1 `FUN_002ee3c0` 277251–277483, B2 `FUN_003c8260`)
Built once per class per view (cached, rebuilt when marked dirty) and drawn for every object of the class through
that object's matrix. The template's sprites go into a max-heap on −(p · cam row 2) — in TEMPLATE space — and are
popped farthest first (the first N − (+0x110) are dropped; none on disc). Sprite i: right = normalize(cam row 0 +
cam row 2·(±0.3·(i % 5))) (− for odd i, 277406), up = cam row 1 — world camera axes used in template space, so the
quads face the camera exactly only on untilted objects, and are stretched along the object's up by S.

### 6.3 The quad (B1 `FUN_002ee030` 277107–277249, B2 `FUN_003c7e28`)
c = p + 0.1·cam row 2; W = right·size, U = up·size; TL = c + U/2 − W/2 + W·sway, TR = TL + W, BL = c − U/2 − W/2,
BR = BL + W (so a square of side `size`, centred on the sprite, its top edge sheared by sway·size); vertex colour =
(grey, grey, grey, Alpha·ramp)·128 on all four (ramp = min(Σ +0x114, 1) = 1 on every disc class). Vertices go out as
(x + 50)·655.35 16-bit fixed point (±50 m from the template origin; B2 `DAT_006c13f0..08` = ∓50).

### 6.4 UVs [V] (tables byte-identical in both ELFs: B1 0x518a80 / 0x518ac0, B2 0x5ffc68 / 0x5ffca8)
u16 (u, v) in 1/2048 per corner, corners TL, TR, BL, BR, indexed by the part byte (i % 4):
- NumParts == 1 → **the whole sheet, rotated by 90°·part** (part 0 identity; 1 = TL→(1,0), TR→(1,1), BL→(0,0),
  BR→(0,1); 2 = 180°; 3 = 270°);
- otherwise → **the sheet's quarters** (part 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right). Only
  `NumParts == 1` is tested.

### 6.5 GS state and colour [V; V measured]
- The leaf state struct (B1 `FUN_002eeba0` 277642–277647, set once; B2 `FUN_003c8fb0` 423779–423784), sent every draw
  by `FUN_0039c1a8` / `FUN_004979a8`, and written or read nowhere else (addrscan): u64 0x100044 (B2 0x104044) — the
  low byte is GS ALPHA A/B/C/D = 0/1/0/1 = (Cs − Cd)·As + Cd, bits 14/20 are outside ALPHA's fields [H: engine
  flags]; TEST 0x5140b = ATE, ATST GEQUAL, AREF 0x40, **AFAIL FB_ONLY**, ZTE, ZTST GEQUAL; u16 0x58 = PRIM IIP | TME |
  ABE (no fog bit). The same register layout reads sensibly in the ten other B1 structs of this kind read (e.g. the
  grass's 0x5160d, `FUN_002ed830` 276723–276727 = GREATER 0x60, FB_ONLY; a glow's 0x31001 = NEVER, FB_ONLY, Z ALWAYS).
- **Proven on the PS2** (`ps2exp/`, `measure.txt`, sheets): TEST → 0x50401 (never pass, AFAIL KEEP) removes the leaves
  and nothing else (|Δ| outside the projected leaf mask 0.00 B1 NAB1, 0.03 B2 DAG1, 0.06 B2 YAV1); PRIM → 0x18 (ABE
  off) turns every leaf quad into an opaque square (black on NAB1); TEST → 0x5040b (AFAIL KEEP: drop the failing
  texels) changes B1 NAB1's near grove by 2.40 in the mask (reload noise 1.66) but removes most of B2 YAV1's canopy
  (leaf-opaque pixels 39 → 86, no leaves 93) — its clusters are fading (NearSceneRange 50..100) and the fade lowers
  the vertex alpha under the test, so they are drawn by the FB_ONLY blend.
- Texture = the class `Texture` (the debug override `DAT_00518b04` is 0 with no writer); the draw colour = colour·128
  with A·fade (GS MODULATE: texture × vertex grey × draw colour); B2 multiplies RGB by `DAT_007c5754`, the HDR
  0.5/1.0 scale the HDR pass doubles back (`re-model-lighting.md` §1: neutral).

### 6.6 Lighting [V measured; formula H]
- BF1: no light term — the draw uploads no light set and the vertices carry no normal. Measured (NAB1 grove, the
  scene lights zeroed): the 30,054 leaf-opaque pixels (41, 53, 45) → (38, 50, 43) while the rest of the frame goes
  (56, 63, 55) → (35, 43, 37) [V measured].
- BF2: the quad writer also stores per-vertex normals (0,127,0) on the top pair and (0,−127,0) on the bottom
  (`FUN_003c8fb0` 423770–423785 → 423166), and the draw uploads the light set buffer `DAT_007bb690`
  (`FUN_005804a0` → `FUN_0057fd90`, 423589–423590) — the buffer `FUN_00597e98` last filled for a model. Measured (DAG1,
  light1 colour and the global set's Top/Bottom zeroed): leaf-opaque pixels (23, 30, 35) → (7, 9, 10), ~70 % lost [V
  measured]. The VU program's exact sum for these vertices and which model's set is left in the buffer are [H]; the
  port's static `Lit` set on the ±Y normals (through the object's matrix) is the build's PROVISIONAL reading.
- The BF2 grass draw (`FUN_003c6e60` 422576–422577) makes the same light-set upload — FOLGG draws BF2 grass unlit
  [H: may be lit too] → card **FOLGB** (with the grass's own GS state, §6.5).

## 7. Wind [V][O]
B1 `FUN_001b4488` (58293–58312; class vtable 0x52d6f8 slot +0xc), B2 `FUN_001cca98` (gated by `DAT_005fc5e8`): every
frame, for each sprite of each built template: phase += dt·(+0x108); sway = sin(phase)·(+0x10c) (sin = a 2048-entry
table). RAM: each class's phases advanced by one common amount since load (e.g. 55.5 s × 0.5 at NAB1), and sway =
sin(phase)·amplitude to 3.5e-6. One template per class → every object of a class sways in step; sprites differ by
their start phase.

## 8. Not drawn by LEAF (follow-ups)

2026-10-29 correction: the tentative sound slot below is a **bird-particle anchor**,
verified through RTTI, Lua bindings and a live BF2 Yavin update. Ground gusts,
impulses and anchor birds are documented in `re-falling-leaves.md`.

> **Superseded for the falling leaves (2026-10-29):** `re-falling-leaves.md` re-read the spawner — slots refill as they
> free, positions are world-axis box samples, the motion is read [D]. The text below is kept as first written.

- **Falling leaves** — the `LeafSpawner` half of `9LeafPatch` (RTTI `6Entity`, `11LeafSpawner`): when an object is first
  drawn it spawns up to byte +0x80 leaf particles at random points in its ellipsoid (`FUN_001b1f00` 57236–57260;
  `FUN_001af8d0` inits one, storing the terrain height under it from `FUN_00473db0`, `re-terrain-heights.md`), from a
  pool of 100 (`FUN_001b0c30`, updated each frame; every 30 s it queries collision layer 0x100 near each camera for
  more leafpatches, 56583). YAV, DAG1 and KAS2 classes set 6. The particle motion and look are unread [H] → new card
  **LEAFF**.
- **The sound slot** (byte +0x81): the draw passes the object's position to `FUN_0019cd78` (B2 `FUN_001b7b28`),
  which keeps the nearest five positions per type — an ambient-sound anchor [H] → noted on card **LEAFF**.
- **Collision**: the +0x50 object serves the spawner's query only (the only `FUN_00161ba0` call with 0x100;
  B2's twin `FUN_001c9468` queries 0x200). Whether any ray cast (AI sight, bolts) includes those layers is [H] — the
  port keeps leaves as decoration (the maintainer: no collision unless the PS2 gives it).

## 9. The port and the build at the research baseline (card LEAF)

Current canopy code is `game/src/viewer/leaf.rs` and `render/src/leaf.rs`; airborne
leaves are separate (`analysis/re-falling-leaves.md`). The following inventory and
build shape record the earlier baseline, not current completion or acceptance.

The research-baseline port [V]: `engine/crates` has 0 hits for `AttachOdf`, `AttachToHardpoint`, 0xa9d0d48b, 0x3be7b80a; `LeafPatch`
appears only in the FOLG `prp_` parser and placement (`mungefs/src/foliage.rs`, `bake/src/foliage.rs`,
`manifest/src/lib.rs`), whose records nothing draws; `bake/src/hardpoints.rs::wants` bakes skeleton nodes only for
vehicle and armed-building bases, and positions only. The leaf classes bake with `geometry: None`.

Build shape [O]:
1. **bake** — per map: every `leafpatch` class's properties (§3, by hash; the vine and box parameters; Seed wrapped
   to u32, or none); the attach list of every prop class (§2.2's replay; the hardpoint's full bind matrix, at least
   rows 1 and 3); world instances of leaf classes are already manifest instances; the FOLG `LeafPatch` records exist.
2. **game** — the template per class exactly as §4 (a Rust port of `template.py`, its RAM vectors as a unit test);
   the sun = the manifest's `sun_dir`; an unseeded class takes a fixed seed (reversible); objects: attach entry ×
   instance matrix → up/position → the §5 matrix, tint from a seeded LCG per object (reversible), MaxDistance; FOLG
   layer patches with their `prp_` distance and FOLG tint.
3. **render** — the grass pass's design (`render/src/grass.rs`: a per-view CPU sprite buffer per class in template
   space, objects as GPU instances): §6.2's sort and twist, §6.3's quad and sway, §6.4's UVs, the §6.1 fades, colour
   = texture × grey × tint (BF2: × the static light on the ±Y normals, PROVISIONAL), blended with the texture's alpha
   × vertex alpha; depth-tested; clusters far to near. The PS2 writes depth only where α ≥ 0.5 and blends the rest
   without it — a two-part draw, or one sorted blend pass without depth writes (the grass pass's choice); **an alpha
   test alone loses BF2 YAV1's fading canopy** (§6.5).
4. **Acceptance** — measured on the leaf-opaque pixels of the patched-state views (the PS2's own with/without-leaves
   pairs: `ps2exp/frames/bf1_nab1g__{ctl,never}`, `bf2_dag1c__…`, `bf2_yav1l__…`, masks from `leafmask.py`): the
   port's leaf-opaque means within ~15 levels of the PS2's; the F24 pair `bf2_yav1/pairs/cp3_landingzone_post1_v3_up`
   canopy box (250,20,640,200) within ~20 levels (PS2 (24,31,8); batch 3's port (136,141,143) — re-measure the port's
   "before" at the build's parent); the NAB2 vines and COR1 planters visible where the walk sheets show them; goldens
   only where a leaf is in view; sim hashes unmoved.

Decisions for the build (reversible): full NumParticles in split-screen (the ÷ view count is a PS2 fill-rate cut);
any fixed seed for unseeded classes; the NearSceneRange fade kept (the single-pass port has no far scene).

## 10. BF1 NAB1's far crowns are the LOWD model, not leaf patches [V measured]
In `bf1_nab1_cp1_cp2_postA_spawn.p2s` the leaf draw never got past its cull (`DAT_005d0010` = 0, the struct at
0x5d0000 all zero; the nearest leaf object is 237.5 m away against MaxDistance 120), yet the frame shows round leafy
crowns at 214–553 m; in the NAB1 grove the TEST patch leaves the mid-distance round trees untouched. The tree's far
model `nab_prop_trees2LOWD` is textured `nab_prop_trees2lores`, a leafy crown billboard
(`tex/bf1_nab1_nab_prop_trees2lores.png`), and the PS2's far scene draws a geometry's LOWD (`re-far-lowd.md` §0); the
port draws LOD0 (bare `nab_prop_trees2brnch` cards) → NAB1's distant "dead forest" (PXWA F38) is card **LOD**'s.

## 11. Checks run (prompt §5) — verbatim in `checks.txt`
1. `fn` on every cited line (both corpora): each sits in the function named.
2. `absent`/ELF scans: `DAT_005d0010` (1 function), `0x5d0000` (1), `DAT_005fc5e8` (3 loads, 0 stores — addrscan +
   widescan), `DAT_005ffc60` (1 load), `DAT_00517848`/`4c` (5 hits each, one function; stores 0x1fc2c8/0x1fc2d0/
   0x1fc300/0x1fc308), `DAT_005fd02c` (1 function; 3 stores), `FUN_00161ba0` (3 calls: 0x100 once, 6 twice); the
   leaf state structs written/read only in the leaf draw (addrscan); the port grep (0 hits ×4).
3. Twins: every B1 function above has its B2 twin opened (class ctor/setter, template, vine, object, draw, per-view
   buffer, quad, state init, wind, FOLG call, attach setter); callers of each (`checks.txt` §3).
4. `word`: "leafpatch" ×2, "grass", "%s %f" ×2, the class/object vtables, both UV tables ×2 games, `DAT_005fc5e8` = 1,
   `DAT_005ffc60` = 1.0; instruction constants by `eedis.py` (1.5, 0.4, 0.1, 655.35, 0.3, 50, 0.2/0.1/0.7, 0.25/0.2/
   0.8/0.5, the state-struct `sd`/`sw`/`sh`).
5. `writers`: `DAT_00517848`, `DAT_0051784c`, `DAT_005d0010`, `DAT_00677660`/`664`, `DAT_005fd02c`, `DAT_005fc5e8`,
   `DAT_005ffc60`, `DAT_007c5754`, `DAT_006c13d8`.
Measured: 23 templates (§4.2), 2,883 objects (§5), the attach replay on 9 states (§2.3), four patched-state views
(28 PCSX2 boots under ci.lock; `ps2exp/`).

**Verifier** (one Opus agent, foreground, given `claims.md` only — 29 claims): **27 SUPPORTED, 2 PARTLY, 0 NOT
SUPPORTED**; it re-ran the RAM checks, the attach replay (and wrote the true cross-class replay,
`verify/attach_persist.py`, 290/290 on DAG1), the measurements, and read the sheets. PARTLY, folded above: A6 (B2: a
class that is not found returns with the global 0; `FUN_00214600` only for a found class whose type check is
nonzero), E1 (the NearSceneRange fade's scale is the camera's cam+0x90, not the class S). Its additions are folded in:
session-random seeds of unseeded classes, the CRC-32 hardpoint hash, the per-class-per-view packet, the view-count
divisor, B2's layer 0x200, the 0x20000 flush, `DAT_00518b04`, the world vines/bushes, and the dead copy constructor
`FUN_001b2410` (nothing calls it).

## 12. Stayed [H]
The BF2 leaf lighting formula and which light set the buffer holds (§6.6); bits 14/20 of the ALPHA word; whether leaf
objects are drawn in the far-scene pass (§6.1); the falling leaves' motion (§8); the sound slot's use; ray casts
against layers 0x100/0x200; the names of the unrecovered hashes (§3); BF2 grass lighting.
