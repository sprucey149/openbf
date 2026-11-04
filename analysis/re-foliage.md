# OPUS RE (2026-09-26) - [O] UNAUDITED
# RE: the terrain foliage — FOLG + `prp_` (what the PS2 plants on the ground, where, and how it draws)

Card **FOLG** (M1), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/re/folg/` —
`census.py` (every `tern`/`FOLG` and `prp_` on both discs), `place.py` (a transcription of the placement, §4),
`livepool.py` (the PS2's live instance pool read from a PCSX2 save state's EE RAM), `pathblock.py` (§9),
`live_instances.json` (2,085 live PS2 instances = test vectors for the build), `checks.txt` (every §5 self-check,
verbatim). Corpora: B1 = `analysis/SWBFRONT.ELF-decomp.c`, B2 = `analysis/SWBFRNT2.ELF-decomp.c`.
Tags: **[V]** read at the cited lines (and re-checked); **[D]** derived from [V] facts; **[H]** hypothesis;
**[O]** load-bearing for the build. "RAM" = measured in the PS2's own memory from a save state.

Supersedes `re-terrain-heights.md` §8 ("FOLG ... at 2× terrain grid resolution" — wrong for BF1, see §2).

## 0. TL;DR

- **FOLG is a 4-bit-per-cell LAYER MASK, not a type map** [V][O]: bit L of a cell's nibble = "layer L grows here".
  The four layers are defined by the map's **`prp_` chunk** (the munged `.prp` foliage file): each `Layer(n)` has a
  `SpreadFactor` and 1..32 `Mesh()` entries picked by `Frequency`; a mesh is a **model** (`File("name", dist)`), a
  **`GrassPatch("class", dist)`** (a sprite cluster class) or a **`LeafPatch("class", dist)`** (LEAF's class).
- **Cells**: BF1's FOLG is always 512×512 at a fixed **16 m** cell centred on the origin; BF2's is (gridSize/2)² at
  **2 × the terrain grid unit** [V][O]. World position of cell (x, z): `x_f = (x − dim/2 + 0.5 + jitter)·cell`,
  world `(x_f, −z_f)` [V][O].
- **Placement is deterministic** [V][O]: a 256-entry float table (identical in both ELFs) seeded per 32 m block and
  layer. `place.py` reproduces **every** live foliage instance in 9 PCSX2 save states (2,610 of 2,610) — BF1 END1
  97/97, KAS1 78/78, YAV1 125/125, NAB1 230/230, KAS2 180/180; BF2 DAG1 261/261, END1 305/305, FEL1 809/809 and
  525/525 at a second pose: x, z to < 1 mm and the yaw to < 1e-4.
- **Instances** sit on the terrain (a model at the height under it and tilted to the terrain normal, with a random
  yaw and a uniform `Scale`; a patch at the mean of 3 samples, tilted, never yawed or scaled), `Stiffness` = how
  upright (0 = terrain normal, 1 = world up) [V][O]. Tint = `min(0.7·LUMI + 0.6 (+0.35 for models), 1) ×
  (1 − ColorVariation·rand)` per channel, the rand seeded by the float bits of x·z [V][O].
- **Fade** [V][O]: a model fades out linearly from its `File` distance d to 1.45·d; a GrassPatch fades over the last
  20 m before its `GrassPatch` distance (view depth). An optional near-fade (`0x08be1de3`) dims models closer than it.
- **GrassPatch** = one shared template of `NumParticles` sprites spread uniformly in a disc of `RadiusFadeMax`, each
  with a size in [MinSize, MaxSize], a grey "darkness" tint in [DarknessMin, DarknessMax], an atlas part (`NumParts`
  tables in the ELF), camera-facing (or fixed-direction "tall" blades), swaying on a sine [V][O]; RAM confirms 5
  templates.
- **A second placement rides in `prp_`** [V][O]: block `0x40b06ff7` puts a prop class every `Distance` metres
  (default 18, authored 20–28) along a named **path** — the tree-billboard lines (`*_prop_treebill*`) of BF1 END1,
  KAS2, NAB2, YAV1, YAV2 (hellbushes) and BF2 END1, FEL1, NAB2, YAV1, and **DAG1's `dag1_prop_hellbush` hedge** along
  `hell_bush` (~32 props).
- **The Dagobah canopy is LEAF, not FOLG** [D]: every DAG1 grand tree / tree group / barrier / vine / hellbush class
  carries `AttachOdf = dag1_prop_leafpatch`; the port draws no leafpatch at all.
- **BF2 split-screen** switches foliage **off** on FEL1/KAS2/YAV1 and cuts its range ×0.6 on DAG1/END1 (a `sky_`
  block read only with ≥ 2 cameras) [V]; single-player is always full. The port keeps full foliage in split-screen
  (a modern-PC call, §10, reversible).
- **The port parses and draws none of this** (0 hits for FOLG / prp_ / grasspatch / leafpatch / SpreadFactor under
  `engine/crates`) — but every foliage and path **model** is already baked (`models/*.glb`, §13).
- **Lighting (§16, card FOLGL, 2026-09-28):** every foliage model draws with the PS2's *static* light set — BF2 the
  normal set without the light regions, BF1 `AmbientColor` and the full sun — which is the port's `Lit` path. FEL1's
  near-black leaves were the port's pre-SUND sun; on main the caladium leaves sit within 1.4 levels of the PS2.

## 1. The data on the discs [V] (`census.py`, reproducible)

`FOLG` payload = u32 byte length + the nibbles. BF1: 131,072 B on every map (512×512). BF2: 2,048 B (grid 128 → 64²) or
8,192 B (grid 256 → 128²). Layers with bits set / prp_ layers defined (a map with no `prp_` draws nothing even with
FOLG bits — BF1 BES1, KAM1 carry a template FOLG and no `prp_`):

| map | FOLG bits set l0/l1/l2/l3 | `prp_` layers → meshes (SpreadFactor) | path block |
|---|---|---|---|
| B1 END1 | 2951/1956/1293/0 | 0 GP end_fern 60 + GP end_prop_grass 60 (0.35) · 1 end_prop_fern5 30 + end_prop_treeclump_1 40 (0.8) · 2 end_prop_foliage5/6/7 80/70/50 (0.8) | jungle1 → end_prop_treebill /28 m |
| B1 KAS1 | 431/381 | 0 GP kas_prop_grass 50 (0.3) · 1 GP kas_prop_reeds 50 (0.3) | — |
| B1 KAS2 | 1389/474 | 0 GP kas_prop_grass_tall 50 (0.1) | jungle1 → kas_prop_treebill /28 m |
| B1 NAB1 | 3804/737 | 0 GP nab1_grass 40 (0.1) · 1 GP nab1_prop_grass_tall 50 (0.2) | — |
| B1 NAB2 | 2 cells | none | jungle1 → nab2_prop_treebill /28 m |
| B1 YAV1 | 286/67/51/77 | 0 GP yav_prop_grass 60+50 (0.3) · 1 yav_prop_shrub1 35 (0.2) · 2 **LP** yav_prop_bush2 35 (0.4) · 3 **LP** yav_prop_bush 40 (0.5) | Jungle1, jungle3, jungle4 → yav_prop_treebillboard |
| B1 YAV2 | 275/39/38/46 | as YAV1 | Jungle1..4 → yav2_prop_hellbush |
| B1 GEO1 | 575 | 0 geo_prop_flatrocks 40 + _2 30 (0.2) (its `Layer(1)` is nested inside layer 0's scope → never loaded) | — |
| B1 RHN2 | 74/47 | 0 rhn_proprocks 35 (0.2) · 1 rhn_proprocks_02 35 (0.1) | — |
| B1 HOT1, RHN1, TAT1 | 0 | rocks / stones defined | — (FOLG empty → nothing) |
| B2 DAG1 | 389 | 0 GP dag1_prop_grass 60+50 (0.3) | hell_bush → dag1_prop_hellbush /28 m (other entries name absent classes) |
| B2 END1 | 1761/1573/1296 | as B1 END1 | jungle1 → end_prop_treebill |
| B2 FEL1 | 808/91/453/696 | 0 fel1_prop_foliage_b 100 (0.2) · 1 fel1_bldg_pitcher_bush 100 (0.7, collision) · 2 fel1_prop_foliage_d 100 (0.3) · 3 GP fel1_prop_thick_bush 95 (0.2) | jungle1 → fel1_prop_hellbush /20 m |
| B2 KAS2 | 270 | 0 GP kas_prop_grass_tall 50 (0.1) | — |
| B2 GEO1 / HOT1 | 51 / 0 | as BF1 | — |
| B2 NAB2 | no FOLG | none | jungle1 → nab2_prop_treebill |
| B2 YAV1 | 286/67/51/77 | as B1 YAV1 | Jungle1, jungle3, jungle4 → yav_prop_treebillboard |

(GP = GrassPatch, LP = LeafPatch; the number after a mesh is its distance.) Map-wide instance counts from `place.py`:
B1 END1 9,151 · KAS1 2,436 · KAS2 13,890 · NAB1 41,725 · YAV1 1,449 · GEO1 2,875 · RHN2 840; B2 END1 6,391 · FEL1 8,970 ·
DAG1 1,167 · KAS2 2,700 — the PS2 only instantiates those near the camera (§5). BF1 and BF2 END1 are the same map: BF1's
512² FOLG is BF2's 128² one shifted by 192 cells at the same 16 m cell (`cmp_end1.png`), plus a stray painted frame
outside the terrain on BF1.

## 2. Loading [V][O]

- **FOLG**: tern loader B1 `FUN_004726b8` 536705–536709 (tag 0x474c4f46 → u32 size, alloc, raw read into
  `DAT_005229b8`); B2 `FUN_0055db58` 694800–694806 (→ `DAT_0060bed8`).
- **Lookup**: B1 `FUN_00344d98` 331498–331514: `b = buf[z·(D>>1) + (x>>1)]`, even x → high nibble; bounds 0..D−1
  else 0; D = `DAT_006270c8`. B2 `FUN_0042b0c0` 485891–485909: `buf[(z·D + x) >> 1]` (same for even D), D =
  `DAT_00700cd8`.
- **Grid** — BF1: `FUN_00343b08` stores `… ELF:
  `lui at,0x4180 / swc1 $f1,0x70cc(s0)` at 0x343b28–0x343b60, `addiu v0,zero,512` at 0x343b5c); these are their only
  writers (text search + `widescan.py`: 1 store each, `checks.txt`). BF2: `FUN_00429640` sets the same defaults
  (484643–484645), then the `prp_` loader `FUN_0042b1a8` overrides them when a FOLG was loaded: cell `DAT_00700cdc =
  2·DAT_00661400` (485956), D `DAT_00700cd8 = DAT_006613fc / 2` (485958), where `DAT_00661400` / `DAT_006613fc` are
  the tern INFO grid unit and grid size (694899 / 694904). No FOLG → foliage flag `DAT_00600bd6 = 0` (485953).
- **`prp_`** (tag 0x5f707270): B1 top-level dispatch `FUN_00175d00` 17434–17435 → `FUN_00344e80`; B2 `FUN_00177058`
  18395–18396 → `FUN_0042b1a8`. Both run the four layer slots (B1 `DAT_00626de0 + i·0x94`, i < 4 — the free loop in
  `FUN_00343bf0` stops at 0x627030; B2 `DAT_007009e0`).

## 3. The `prp_` grammar [V][O]

A munged config: `DATA` = u32 FNV-1a(lower-case name) hash, u8 arg count, 4-byte args (floats; a string argument is
stored as its FNV hash, except `Lighting`'s which the loader reads as text); `SCOP` = the children of the preceding
DATA. Names recovered by hashing (`veh0/fnv.py`); the ELF only holds the hashes.

- `Layer(i)` 0x07a640f6 (B1 331557–331567, B2 485969–485975) → layer i's `Mesh`/`SpreadFactor` loader B1
  `FUN_00345980`, B2 `FUN_0042bc40`. Nested `Layer` inside a layer scope is ignored (GEO1).
- `SpreadFactor(v)` 0xdbe276e9 (B1 332035–332040; ELF 0x345ac0–0x345af8 `cvt.w.s`, `div.s $f20(=1.0)`): `spread =
  trunc(v)`; if that is 0: `spread = 1` and, if v > 0, `count = trunc(1/v)`. Defaults spread 1, count 1 (332012).
  So v ≥ 1 → one instance per spread×spread cells; v < 1 → trunc(1/v) instances per cell. (EE `cvt.w.s` truncates.)
- `Mesh()` 0xa100bebc → a 0x50-B entry (`FUN_00346e70` / B2 `FUN_0042d0b0`), loaded by B1 `FUN_00346f50`
  332836–333034 / B2 `FUN_0042d1a0` 487252–487475:

| property | hash | stored | meaning |
|---|---|---|---|
| `File(name, d)` | 0xaaea5743 | model (by hash in the model table), `[6]` = d ≤ 1000 (default 1000) | kind 1 = a model (332940–332957; resolve 333001–333008) |
| `GrassPatch(class, d)` | 0x156b70a1 | `[0xb]` class hash, `[6]` = d, kind 2 (332920–332934) | a `grasspatch` class instance (§7); its `File` is an editor placeholder |
| `LeafPatch(class, d)` | 0x6a6fb399 | kind 3 (332880–332894) | a `leafpatch` class instance (LEAF) |
| `Frequency(f)` | 0x2fb31c01 | `[1]` (base `FUN_00346138` 332307) | pick weight; normalised to a cumulative 0..1 (332060–332082) |
| `Scale(s)` | 0x82971c71 | `[8]` (332978–332981) | uniform scale |
| `Stiffness(k)` | 0x495f91ae | `[2]` (332312) | up = lerp(terrain normal, world up, k) (§6) |
| `ColorVariation(c)` | 0xbbce475d | `[0x12]` (332983–332986) | per-channel random darkening (§6) |
| `Lighting("dynamic"/"static")` | 0x827eb1c9 | `[0x13]` = 1 only for "dynamic" (332959–332976; strings 0x545858/0x545860) | draw flag 0x80 when static (§6) |
| `Sound(name, min, range)` | 0x0e0d9594 | `[0xc]`, `[0xe]`, `[0xf]` | an ambient sound replayed at a random interval (§11) |
| `0xc28f0c96(name)` | 0xc28f0c96 | `[0x10]` | the sound played when a soldier walks into the instance (§11) |
| `0x08be1de3(n)` | 0x08be1de3 | `[7]` | the near-fade distance (§6) |
| `0x26edc40c()` | 0x26edc40c | flag | build the model's `CollisionMesh` (string 0x545868; 332936, 333011–333027) |
| `0x13fc0734(a, b)` (BF2 only) | 0x13fc0734 | `[0x14]`,`[0x15]` (487316–487320) | handed to the soldier controller (§11); not drawn |

BF2's FEL1 writes `Lighting(1.0)` — a float where the loader reads a string, so it stays "static" [D] (no map uses
"dynamic" as text; the compare is a case-insensitive prefix test of the argument against "dynamic", verifier B5). The
entry's `[0]` = its cull distance: `d·1.45 + model radius·Scale` for a model (333007), for a patch `d·1.45 + class
bound·Scale` once the class is resolved (`FUN_003474c0` 333086–333088) — but the layer maximum is taken at load, when a
patch's `[0]` is still `d` (verifier B3/D2). The base property 0x39f85b7d → `[0]` is dead: `FUN_00346f50` always
rewrites `[0]` at its end. A `File`'s distance counts only while no GrassPatch/LeafPatch was given (0x347118).

## 4. Placement — deterministic [V][O] (RAM-verified)

- Blocks of 2×2 cells, `D8 = (D+1)/2` per side (B1 331612; B2 `FUN_0042b330`), block size = 2·cell (331610).
- Per block (bx, bz) and layer L whose mask bit is set: seed the table RNG with `bz·D8 + bx + L + 2·bz`
  (B1 `FUN_00344a68` 331409, B2 `FUN_0042add0` 485790); `rand()` = `table[++i & 0xff]` (B1 `FUN_00345480` 331738–331739,
  table 0x5453c8; B2 `FUN_0042b710` 486124, table 0x636ac8 — the same 256 floats, `re_check word`).
- Cells z = 2bz, 2bz+1 (outer), x = 2bx, 2bx+1 (inner); for a cell with bit L, `count` times (B1 `FUN_00345c78`
  332107–332157, B2 `FUN_0042bf00` 486524–486573): if spread > 1 and (x % s or z % s) → skip **without** a rand; r =
  rand(); the first mesh with `r ≤ cumulative` wins; then rx, rz, ryaw = rand() ×3;
  `x_f = ((x − D/2) + 0.5 − 0.45·s + 0.9·s·rx)·cell`, same for z; `yaw = 6.1415·ryaw` (sic — not 2π).
- The instance stores x at +0x50 and **−z_f** at +0x58 (B1 `FUN_00346480` 332465–332466; B2 `FUN_0042c648` 486865–486866);
  the camera sweep indexes blocks with `(x, −z)` too (331331). So world = (x_f, h, −z_f); "RAM coordinates ARE the port's
  world coordinates" (`engine/artifacts/pcsx2/README.md`).
- **RAM check** (`livepool.py`): BF1 pool `DAT_006270f0` 1000 × 0x70 (free stack `DAT_00642670`, count `DAT_00519110`,
  `FUN_003462b8` 332344–332365); BF2 pool `DAT_00700d00` 0x380 × 0x70 (`FUN_0042c468` 486750–486765). Every live
  instance of 8 states equals a predicted one (x, z < 1 mm, yaw < 1e-4): 2,085 of 2,085 (§0).

## 5. Streaming and draw distance [V]

- The camera sweep (B1 `FUN_00343f80`; B2 `FUN_0042a228`) walks the view frustum out to `2·block + 100` m with one
  camera, `block + 50` m with two or more (B1 331281/331284; B2 485664), and assigns free blocks (at most
  `min((100/block + 5)², 254)`, B1 331665–331667) to in-view cells.
- Per block per layer (B1 `FUN_00343148` 330239–330326, B2 `FUN_00428be8`): with `depth` = the block centre's distance
  along the camera's horizontal forward, the layer's instances are built and drawn while `depth < layerMax + r` and
  freed beyond `1.05·layerMax + r` (330289–330318), r = 0.707·block. `layerMax` = the largest `[0]` of its meshes
  (`FUN_00345540` 331783 returns layer+0x8c, set at 332031).
- Per instance (B1 `FUN_003465c0` 332493–332746; B2 `FUN_0042c788`): drawn if its cull distance `[0]` (the vtable getter
  0x346e60 = `lw v0,0x14(a0); lwc1 $f0,0(v0)`, a leaf the decompiler dropped) exceeds the block's near depth and the
  camera distance; then by kind (§6, §7).
- The pools cap the PS2 (1000 / 896 instances; FEL1's spawn state has 809 of 896 live). **The port need not stream or
  cap** — it can draw every placed instance inside its distance (§13 decision).

## 6. The instance: transform, tint, fade [V][O]

- **Transform** (B1 `FUN_00347610`, B2 `FUN_0042d960`): model → y = terrain height at (x, z) and up = the terrain normal
  (`FUN_003454b0` 333186); patch → y = mean of three samples at (x, z+3), (x∓2.25, z−2.25) (333194–333226; BF2 (x, z+4),
  (x∓3, z−3) 487652–) and up = that triangle's normal. Then `Stiffness` k > 0: up = normal·(1−k) + (0,1,0)·k
  (333250–333253); **models only** (the `bne v1,1` at 0x347844 skips it for patches): rotation by yaw about up
  (sin/cos of `[+0x18]`, 333305) and the uniform `Scale` (333269). A patch is positioned and tilted, never yawed or
  scaled (verifier D6).
- **Tint** (B1 `FUN_00346b70` 332750–332796, B2 `FUN_0042cdb0` 487164–487208): `L` = bilinear LUMI/255 at (x, z)
  (`FUN_00473bc0` / B2 `FUN_0055f068`, `re-terrain-lighting.md` §4); models add 0.5 (332769);
  `c = min(0.7·L + 0.6, 1)` (332772); each of R, G, B = `trunc(c·(1 − ColorVariation·u)·255)`, u from the LCG
  `0x19660d/0x3c6ef35f` (steps 3–4 → R, 5–6 → G, 7–8 → B) seeded with the **float32 bit pattern** of `x·z` xor
  0xc71c2830 — the decomp's `(uint)` is a raw move (ELF 0x346be8 `mul.s`, 0x346c08 `mfc1`, 0x346c20 `xor`; verifier D5),
  the product in the EE's round-toward-zero, x and z the settled +0x50/+0x58 (after `FUN_00347610`'s matrix rebuild).
  Reproduced by the lead (`tintcheck.py`): on 591 settled instances with ColorVariation ≠ 0 the chop-product seed fits
  all three bytes 591/591 (round-to-nearest 286, integer conversion 2). A = 255 (332783). RAM: models 255 (clamped)
  unless ColorVariation, KAS1 grass (204,204,204).
- **Model fade** (332623–332667), with `dist` the **horizontal** (x, z) distance to the camera (VMUL.xz at 0x346638):
  skip beyond `1.45·d` (when d > 0); alpha 1 → 0 linearly over [d, 1.45·d]; inside d, if `[7]` > 0:
  alpha = `max(dist/[7], 0.1)²` closer than `[7]`, else `DAT_00519114` = **0.999** (→ 254: RAM #905 END1); alpha 255 when
  `[7]` = 0. Draw flags: `| 0x80` when static (332675), `| 0x800000` (BF1 332682) / `| 0x1000000` (BF2 487148) — their
  meaning is read in §16 [O] (0x80 = the static light set; the other flag changes no light term). In BF2 any draw colour
  with A ≠ 255 takes the translucent path (`re-side-alpha.md` §3) — a fading or near-faded model blends [D].

## 7. GrassPatch — the sprite clusters [V][O] (RAM-verified templates)

- **Class** `grasspatch` (factory B1 `FUN_001ad820` 54514; B2 setter `FUN_001c65f0` 72772, same hashes, fields +0x20,
  plus an unused flag 0x9c3f7a19). Setter B1 `FUN_001adcd8` 54679–54806 → fields (defaults from the ctor 54532–54557):
  `NumParticles` 0xd133f7da +0x84 (50; **divided by the camera count**, 54784) · `MinSize` 0xeaebcf6e +0x88 (10) ·
  `MaxSize` 0x80ba1308 +0x8c (15) · y offset 0x3d5d0aef +0x90 (0) · `Alpha` 0x5d8b6dab +0x94 (0.5) · `MaxDistance`
  0xe6915cf6 +0x98 (100) · `Texture` 0x3c6468f4 +0x9c ("grass", 0x52d418) · `RadiusFadeMin` 0x80887a6f +0xa0 ·
  `RadiusFadeMax` 0x6e74d661 +0xa4 · `DarknessMin` 0x13fac25e +0xa8 (1) · `DarknessMax` 0x25e5c3fc +0xac (1) ·
  `NumParts` 0x5ac02207 +0xb0 (4) · rectangle 0x35a4393b +0xb4/+0xb8 (−1: use the disc) · 0x231fcc79 +0xbc/+0xc0 ·
  0xa9b14958 +0xc4 (2.0, tall-size factor) · 0xf6582b9c +0xc8 (0.5, tall lean) · 0xc75263e1 +0xcc (ground-shadow quad) ·
  0x97133eeb +0xd0 (0.1, static lean) · 0x4b6d5171 +0xd4 (6) · 0xff7bb39a +0xd8 (0.3) · 0x604db82e +0xdc flag.
- **Template, once per class** (`FUN_001ae120` 54828–55230): N sprites; position uniform in a disc of `RadiusFadeMax`
  (rejection from the square, 54915–54942) — or, when `0x35a4393b` is set, `(u − 0.5)·(w, d)` in that rectangle with
  f = 1 and no redraw (54893–54906) — with falloff f = 1 inside `RadiusFadeMin`,
  linear to 0 at `RadiusFadeMax` (54937); size `MinSize + u·(MaxSize − MinSize)` (54947–54950); grey `DarknessMin + u·(Max −
  Min)` and `Alpha` (54951–54962); sway phase `u·10` (55006–55007). Parts by `NumParts`: **4 (and 1, 2)** → part `i % 4`,
  width ×1.1 / height ×0.9, sunk by `MinSize·(1−f)·0.5` (55138–55150); **3** → `i % (d4+2)`, parts ≥ 2 become "tall"
  blades with part **2** (ELF 0x1aea5c `addiu v0,zero,2 / sb v0,60(v1)` — the decompiled `= 0` at 55025 is a
  delay-slot artefact); **≥ 5** → `i % (d4+4)` folded below 6, parts > 3 tall. Tall blades: size ×0xc4, a fixed random
  facing (+0x20), raised by `MinSize·(bc + u·(c0−bc))·(1−f)`.
  RAM (5 classes, END1/KAS1/NAB1): radius max ≤ RadiusFadeMax, sizes ×1.1/×0.9, sink ≤ MinSize/2, parts {0..3} equal for
  NumParts 4 and 1, {0:4,1:3,2:18} with 18 tall for end_fern (NumParts 3), {0..3:6, 4:18, 5:18} with 36 tall for
  nab1_grass (NumParts 6).
- **Atlas** [V] (.data 0x518948–0x518a57, never written — ELF scans; u16 u,v in 1/2048 per corner, 4 corners per
  part): NumParts 3 @0x518948 = {u .75–1 v 0–.5,
  u .75–1 v .5–1, u 0–.75 v 0–1}; 4 @0x5189d8 = the four quarters; 1 @0x518a18 = the whole sheet, every second part
  u-mirrored; other @0x518978 = four u-quarters of the top half + two u-halves of the bottom half.
- **Draw** (`FUN_002ed080` 276429–276465): with `depth` = the patch's view depth, draw if `−bound ≤ depth ≤ D`; alpha ×=
  `min((D − depth)·0.05, 1)` — **a 20 m fade before D** (276457–276460). D = the `prp_` GrassPatch distance for a FOLG
  instance (332610) and the class `MaxDistance` for a grasspatch placed as a world object, drawn white (`FUN_001af3e0`
  55299 — SKIPC's `nab_prop_flowers`, `yav_prop_grass(_tall)`). Texture = the class `Texture` (276506; the debug
  override `DAT_00518a6c` is 0 and never written). The per-view buffer is rebuilt back-to-front (`FUN_002eca58`); each
  sprite quad (`FUN_002ec4c8` 275926–276180): normal sprites stand on their base point, right = camera right rotated by
  ±0.05·(i%5) toward the view axis, up = the tilted up `normalize(Y − 0.7·(Y·fwd)·fwd)` (276230–276251), top edge
  sheared by `(sin(phase)·0.08 + lean)·width` (276007); tall blades face their fixed direction and lean away from the
  camera by `0xf6582b9c + sin(phase)·0.03`, with an optional darker (×0.75) ground quad when `0xc75263e1` ≠ 0. Vertex
  colour = grey × 128 per sprite; the instance tint and alpha are the draw colour.
- **Wind** (`FUN_001af2e0` 55254–55282, class vtable 0x52d468 slot +0xc): each frame `phase += dt·0.8·(1 + 0.2·(i%5))`
  (`DAT_005174b4` = 0.8), `sway = sin(phase)`; one template → every patch of a class sways in step.
- [H]: the GS blend of the sprites (the texture's alpha, the fade in the vertex/draw alpha — so blended, not only
  tested); ~~the exact corner ↔ UV order~~ (read below).
- **Build notes (card FOLGG, 2026-09-26 — the builder's own readings, NOT verifier-checked):**
  - Corner order: `FUN_002ec2d8` 275862–275925 writes v0 = the top-left point, v1 = v0 + W, v2 = base − W/2, v3 =
    base + W/2 and pairs vertex k with the part's k-th (u, v): atlas corner order is top-left, top-right, bottom-left,
    bottom-right (v = 0 is the sheet's top).
  - BF2's atlas is the same 0x110 bytes at 0x5ffb20 (1 → 0x5ffbf0, 3 → 0x5ffb20, 4 → 0x5ffbb0, else 0x5ffb50; B2
    `FUN_003c6788` 422412–422424); BF2's class fields sit +0x20 later (NumParts +0xd0, tall lean +0xe8, flag +0xfc).
  - A "tall" sprite is a FLAT quad: `FUN_002ec4c8`'s else-branch spans it by side = facing × up′ and side × up′, both
    ⊥ up′ = Y − L·lean (L = the horizontal look axis *(decompiler detail omitted)* of `FUN_002eca58`), centred on its point. The
    sheets agree: `end_fern`'s part 2 is a top-down fern rosette, `nab1_grass`'s lower half flat ground cover.
    `0x604db82e` set drops the `sin(phase)·0.03` from that lean (the *(decompiler detail omitted)* test).
  - For NumParts 3 / ≥ 5 a normal sprite is `size` high, `size·0xff7bb39a` wide, its up axis × (0.8 + 0.2u) (+0x54,
    the lerp weight of the tilted up against a unit vector — a scale in effect); +0x58 = 10000 there is unread.
  - The patch's matrix (333399–333460): row 1 = the (Stiffness-lerped, not renormalised) up, row 0 =
    normalize((0,0,−1) × up), row 2 = normalize(row 0 × up); position − up·mesh[9] (0 on the disc).
  - The draw's camera row 2 is the BACK axis: `FUN_002ed080` culls on `(cam − p)·row2 ≤ D`, so depth > 0 ahead.

## 8. LeafPatch layers → LEAF

`LeafPatch(class, d)` layers exist only on BF1 YAV1/YAV2 (`yav_prop_bush`, `yav_prop_bush2`, 256 instances map-wide).
They are placed exactly as §4 and drawn by the leafpatch class (`FUN_001b38f8` resolve 333083; draw `FUN_002ee970`
332731) — LEAF's renderer. FOLG's bake places them; LEAF draws them. **Correction (LEAF RE, 2026-09-29):** BF2 YAV1's
`prp_` carries the same two layers (the port's bf2_yav1 manifest lists them); the leafpatch system is in
`re-leafpatch.md`.

## 9. Path placement — the `0x40b06ff7` block [V][O]

B1 `FUN_00344e80` 331568–331571 → `FUN_003489b0` 334277–334310: each child `0x84874d36(pathName)` (334295) →
`FUN_003483f8(pathHash, scope)` 333927–334274: `0x6850acc6(class)` appends an entity class (≤ 12; only if the class
exists, 334040–334058); `0xef838cc7(class)` a second list (never used); `0xbad71c66` flips the facing (334062,
334174); `Distance` 0x2eb31462 (334089; default **18**, 334032). The path comes from the PATH table
(`FUN_00472290(0x676110, hash)`, 334099). Walking node to node, while the next node is farther than `Distance`, it
steps `Distance` along the segment and creates a random class of the list (global LCG `DAT_0066eec8`, an unsigned
`divu` modulo; one class on every disc entry → deterministic) with its matrix at the step's **midpoint**, upright,
facing perpendicular to the path (334128–334266). The objects are ordinary props (their ODF's model, leafpatch attachments, collision). B2 twin
`FUN_0042e838` (called from `FUN_0042b1a8` 485981). Census (`pathblock.py`, manifest paths for BF2): B1 END1 jungle1
2,409 m → ~86 `end_prop_treebill`; KAS2 986 m → ~35 `kas_prop_treebill`; NAB2 559 m → ~19; YAV1 3 paths → ~61
`yav_prop_treebillboard` (3 more entries name absent paths); YAV2 4 paths → ~80 `yav2_prop_hellbush`; B2 DAG1
`hell_bush` 909 m → ~32 `dag1_prop_hellbush`; END1, FEL1 (1,134 m /20 m → ~56 `fel1_prop_hellbush`), NAB2, YAV1 likewise.
Entries whose path or class is absent place nothing (DAG1's four YAV-copied entries).

## 10. Split-screen and switches [V]

- BF2 `sky_` → `SkyInfo` property `0x23e10337()` is a scope (`FUN_003be120` 416934–416940 → `FUN_003be658`) whose
  children are applied **only when `DAT_00600b30` ≥ 2** (417087); `DAT_00600b30` is the camera count (default 1 in
  .data; `ScriptCB_SetSplitscreen` 42814, `ScriptCB_GetNumCameras` 42824, `ScriptCB_IsSplitscreen` 42834; also set by
  the demo/net launch paths and `FUN_00170770`, all listed in `checks.txt`). Inside:
  `0xb1bfdd49(0)` → `FUN_0042c440(0)` foliage off (417008), `0x91b20cc9(v)` → the distance scale `DAT_00600bf8`
  (417052), `0x30c322fe(v)` → the density scale `DAT_00600bfc` (416960). Disc: FEL1, KAS2, YAV1 off; DAG1, END1, FEL1,
  YAV1 distance 0.6; DAG1 density 0.1.
- Defaults (ELF .data, `re_check word`): `DAT_00600bf6` = 1 (on), `DAT_00600bf8` = 1.0, `DAT_00600bfc` = 10.0,
  `DAT_00600c00..c05` = 1 (all cull/fade paths on; no writers), `DAT_00600c08` = 0.999. The density scale feeds a
  random thinning in `FUN_0042d750` 487540–487550 that keeps every instance at 10.0 [D: 10·max(…, 0.3) ≥ 3 > rand].
- BF1: `NumParticles / cameras` (54784) and the smaller sweep radius (331284) are its split-screen cuts.
- **Port call (reversible):** keep full foliage in split-screen — these are PS2 fill-rate cuts, the "hardware limit we
  improve on" rule. Flip it by honouring `0x23e10337` when the port has ≥ 2 views.

## 11. Gameplay side-effects (not drawn) [V]

- Walking into an instance: B1 `FUN_00221a70` 135806–135838 → `FUN_00345560` (the instance whose bound holds the soldier,
  blocks within 30 m, 331820) and plays its `0xc28f0c96` sound on entry (135826). B2 `FUN_00269860` 186235–186280 does
  the same and passes the entry's `0x13fc0734` pair (default 0.8, 1.0) to `FUN_002ae9f8` → controller +0x68/+0x6c
  (186261–186274) — [H] a concealment/visibility range; out of FOLG's scope (an M3 card if it matters).
- `Sound(name, min, range)`: an ambient emitter per instance replayed every min + u·range s (`FUN_00346508` 332476–332490,
  the timer at *(decompiler detail omitted)* 332736–332745). `0x26edc40c`: the model's `CollisionMesh` becomes a collision object (FEL1 pitcher bush, GEO1 flat
  rocks) — **the only foliage with collision** (the maintainer: walk-through unless the PS2 gives collision). They are built
  map-wide once, at load: the end of the `prp_` load (`FUN_00345008` → `FUN_00345e30` → entry slot +0x10 =
  `FUN_00347b68`) replays the same placement with the same seeds over every cell, so the solid spots are exactly the
  visible ones (verifier, "outside the claims"; gated by `DAT_00518be4`/`DAT_00518be8` == 0, their .data default).

## 12. What each map gets (the build's checklist)

Models (existing `.glb`, instanced): B1 END1, YAV1/YAV2 (shrub1), GEO1, RHN2; B2 END1, FEL1, GEO1. GrassPatch: B1 END1,
KAS1, KAS2, NAB1, YAV1, YAV2; B2 DAG1, END1, FEL1, KAS2, YAV1. LeafPatch (LEAF): B1 YAV1, YAV2. Path props: B1 END1,
KAS2, NAB2, YAV1, YAV2; B2 DAG1, END1, FEL1, NAB2, YAV1. Nothing: HOT1, TAT1, RHN1, BES*, KAM1, TAT2, COR1, DEA1, GAL1,
KAM1, MUS1, MYG1, POL1, TAN1, UTA1 (no FOLG bits or no `prp_`).

## 13. The port and the build plan

The port: 0 hits for FOLG / prp_ / grasspatch / leafpatch / SpreadFactor / 0x7a640f6 / 0x40b06ff7 over the 200 `.rs`
files of `engine/crates` (main 520c4cc; `checks.txt`). Every model a layer or path names is already baked
(`assets_baked/<map>/models/*.glb`, all 22 checked). `mungefs::config::parse_tree` reads munged configs;
`mungefs::path` already bakes PATH/path chunks into the manifest.

1. **FOLG (build)** — bake + models + path props: parse `prp_` (layers, meshes, path block) and FOLG; run §4 exactly
   (the table from the ELF → a 256-float constant); y, up, yaw, scale, tint from the baked terrain (§6); emit per map a
   foliage instance list (kind, mesh/class, x, y, z, up, yaw, scale, rgba, distance, near-fade); path props as extra
   world instances of their class (§9). Draw model instances with the existing model pipeline + the §6 fade/alpha.
   Acceptance: a unit test reproduces `live_instances.json` (x, z to < 1 mm, yaw to 1e-4; y within a measured
   tolerance; rgba exactly when the test feeds the RAM x, z into the tint — the port's own placement need not be
   bit-exact, so its per-instance tints may differ from the PS2's while keeping the same spread, a reversible call);
   the pairs `bf2_fel1 cp3_cp3_1_postA_spawn`, `bf1_end1 cp1_cp2_postA_spawn` show the plants.
2. **FOLGG (new)** — the grasspatch sprite renderer (§7): template per class, atlas, camera-facing + tall blades, sway,
   20 m fade, tint; serves FOLG layers and world-object grasspatch classes (SKIPC's flowers/grass). deps FOLG.
3. **LEAF** (existing) — adds the two YAV `LeafPatch` layers (§8) and the path hellbushes' attachments.

Decisions (reversible): no streaming/pool cap (draw all within distance); full foliage in split-screen; the table RNG
is copied, the global-LCG choices (path class when > 1, grass template layout) use a fixed seed.

## 14. Checks run (prompt §5) — verbatim in `checks.txt`

1. `re_check.py fn` on every cited line (both corpora): each sits in the function named. 2. `absent`: `DAT_005229b8`
(2 hits: loader, lookup), `DAT_0060bed8` (5), `0x5f707270`, `0x40b06ff7` (1 function each game), `0x13fc0734` (B2 only),
`DAT_00519114` (1 read), `FUN_00346e60` (0 — decompiler-dropped, read in the ELF), `0x9c3f7a19` (setter only). ELF:
`widescan`/`addrscan` on 0x6270cc/0x6270c8 (1 store each) and on the UV tables (4 reads, no writer). 3. Twins opened:
every BF1 function above has its BF2 twin read (FOLG lookup, prp_ loader, SpreadFactor, Mesh loader, placement,
instance init, tint, transform, draw, pool) — the differences are listed (§2, §5, §6, §7, §10, §11). 4. `word`: 16.0
and 512 (disassembly), the RNG tables (equal), 0.999, 0.8, 0.08, 0.03, the BF2 defaults, the strings, the Mesh vtable,
the UV tables. 5. `writers`: the grid globals (1 each), `DAT_00519108`, the BF2 switches (only the split-screen setters;
c00–c08 none), `DAT_00600b30` (6: the split-screen/demo/net setters). Data: `census.py`, `place.py`, `livepool.py`
(RAM, 2,085/2,085), `pathblock.py`.
Stayed [H]: draw flags 0x800000 / 0x1000000 / 0x80 meaning (read since, §16); sprite blend and corner order; the `0x13fc0734` consumer;
the names of 0x08be1de3, 0xc28f0c96, 0x26edc40c, 0x40b06ff7, 0x84874d36, 0x6850acc6, 0xbad71c66.

**Verifier** (one Opus agent, foreground, given `claims.md` only — 35 claims A1–I1): **30 SUPPORTED, 3 PARTLY, 0 NOT
SUPPORTED**; it re-ran `livepool.py` (97/97, 809/809) and broke `place.py` four ways as negative controls (seed without
+2bz → 0/97 and 0/809; rx/rz swapped → 0/97 and 28/809; post-increment → 17/97 and 159/809; z not negated → 0/97 and
0/809). The PARTLYs are folded above: D5 the tint seed is the float bit pattern of x·z (reproduced by the lead,
`tintcheck.py`, 591/591), D6 yaw and Scale are models-only, E4 the never-written range is 0x518948–0x518a57
(0x518940/44 and the counter 0x518a58 are written by other code). Refinements folded: the horizontal fade distance,
the dead 0x39f85b7d, the patch layer maximum at load, the load-time collision pass, the unsigned path-class modulo, the
case-insensitive `Lighting` prefix test, `DAT_00600b30`'s default 1.

## 15. The card's questions, answered

- *What does a nibble select?* Four independent layer bits (§0, §3). *Callers of FUN_00344d98*: the load pass
  (`FUN_00345008`), the block mask (`FUN_003453b0`), the per-block build (`FUN_00344a68`).
- *Density* §3/§4; *draw distance / fade* §5–§7; *wind* §7 (grass only; models don't sway — `Stiffness` is uprightness).
- *BF2's twin* opened: same system; FOLG sized from the terrain (§2), extra BF2 switches (§10), one BF2-only property.
- *The Dagobah canopy*: LEAF (DAG1's trees attach `dag1_prop_leafpatch`); FOLG adds DAG1's fern/grass carpet
  (`dag1_prop_grass`) and the `hell_bush` hedge (§9). Not a MASK-cutoff loss.
- The maintainer's INBOX (b), BF1 Kashyyyk's extra trees: KAS2's `jungle1` line of ~35 `kas_prop_treebill` billboards is this
  path block → FOLG's build; KAS1 has none. The leafy crowns are LEAF's.

## 16. The draw flags and the leaves' lighting — OPUS RE (2026-09-28) - [O] UNAUDITED

Card **FOLGL** (M1), loop lane 2, effort max. Scratch (gitignored): `engine/artifacts/lead_tmp/re/folgl/` — `claims.md`,
`checks.txt` (every self-check verbatim), `verify/VERDICTS.md`, `rows.py`, `normdump.py`, `measure.py`, `port/` (the
renders), `ps2exp/` (BF1 patched states). It answers §6's [H] on the draw flags.

**The answer.** The FOLG strip's near-black FEL1 leaves were the port's pre-SUND sun, not a foliage light path [V
measured]. FOLG (9b8d040, 11:29) was rendered before SUND (e328300, 13:02) made `sun_dir` the PS2's own vector. On main
the caladium leaves sit within 1.4 levels of the PS2 (table below). The flags select no prelit, unlit or two-sided path.
The port's `Lit` path is the PS2's static set on both games, so **the port owes no foliage code**.

1. **0x80 is the "static" light set** [V][O]. It is set when the entry's `Lighting` is not "dynamic" (§3). No disc
   authors "dynamic" (verifier C1: all 323 LVLs), so every foliage model carries 0x80.
   - **BF2.** `FUN_0042c788` 487140–487148 calls the model's vtable 0x64feb0 (installed 712517 / 712549). Its slot +0xc
     (ELF 0x64febc) is `FUN_0057cf60`, which calls `FUN_00597e98(sphere, 4, flags >> 7 & 1, model+0x48 & 1, model+0xad)`
     (712781) and uploads the result with `FUN_0057fd90`.
     - The static argument is read once, at 729394. When set, it skips `FUN_0059b2f8` (729423), the light-REGION
       lookup: a list of region volumes (head `DAT_007cd51c`, weights by `FUN_005adc48`) that scales Light1 / Light2 and
       blends the Top/Bottom ambient toward each region's own (731344–; verifier A6). Light1 / Light2 are then added at
       their global fades.
     - The rest is the normal set: hemisphere Top/Bottom, Light1, Light2 and the point lights. The prelit rule (black
       ambient, `Static()` lights skipped, `re-model-lighting.md` §3) keys on the model's own 0x200 bit only: +0xa3 comes
       from argument 4 (729490 / 729907).
     - FEL1's region list is empty in RAM, so there static and dynamic draws light alike. It is non-empty on GEO1 (2–3)
       and END1 (1) (verifier), where static foliage skips it. The port has no regions (card LREG).
     - BF2 ODF classes take the same bit from their own `Lighting` ("dynamic" / "static", strings 0x61c510 / 0x61c518):
       `FUN_00214600` 127897–127911 sets class +0x130, and `FUN_00239bb0` 153340–153341 ORs 0x80 when it is 0.
   - **BF1.** `FUN_003465c0` 332675 calls vtable 0x558fe8 (557008 / 557038), slot +0xc = `FUN_00496248` (ELF 0x558ff4).
     It calls `FUN_00493cf0(…, 4, 1, 0)`, then forces the set's sun factor to 1.0 (557190–557191).
     - The result is ambient `DAT_0069c570` = SkyInfo `AmbientColor`, no active-region walk, full sun (556087–556088).
     - A non-static draw takes `CharacterAmbientColor` or `VehicleAmbientColor` instead (556042–556046). See
       `re-model-lighting.md` §12: BF1 soldiers do, the port does not (card CAMB).
2. **The per-game flag (BF1 0x800000, BF2 0x1000000) changes no light term** [V][O].
   - **BF2:** nothing on the rigid path reads it. The disassembly shows that `FUN_0057cf60`'s masks (0x4000000, 0x20000,
     0x80000010, 0x4020000, 0x230000) and `FUN_0058ed70`'s (0x800000, 0x4000000, 0x20000, 0x2000000, 0x80000, 0x20020,
     0x20, 0x10, 0x1) never include bit 24. The flags are neither stored nor passed on. The bit is cleared for the
     per-bone skinned draw (712920).
   - **BF1:** it BATCHES the segment. `FUN_004cf6f0` 594271–594296 and `FUN_004d0108` 594593–594613 hand an opaque
     single-texture segment to `FUN_004b1688`, a texture-sorted queue at 0x701570 / 0x701580. The gates `DAT_00523014` /
     `DAT_00523020` are 1 in the ELF and have no writer.
     - The flush `FUN_004b1a20` lights each record with its own ambient word, sun factor and point lights (573192–573237):
       the same terms as the unbatched draw. The 0x701580 queue carries no point lights and a factor of 1.0 (verifier B5).
     - Its purpose (fewer GS state changes) is [H].
3. **One-sided lighting on both** [V][O]. BF2 VU1 computes Σ Cᵢ·mat·max(0, n·lᵢ) with the stored normal (0x1f4–0x20d).
   The lights come into model space from the matrix rows (0x72e–0x73e, rows from 0x3d1–0x3e7). The dispatch table
   (0x01d–0x02b) has four targets and no two-sided variant; its pairs differ only in flag 8, which `FUN_0057fd90` always
   sets (verifier A8). The port's world shader lights the same stored normal with no facing flip, so a leaf seen from its
   back shows its front's light on both.
4. **The instance data agree** [V measured].
   - All 504 live FEL1 model instances' rows equal the port's construction (`rows.py`; verifier max error 1e-6), with
     vtable 0x64feb0.
   - The sun vector in RAM, *(`DAT_0060c6e0`)+0x50 = (−0.246, −0.748, −0.617), is the manifest's `sun_dir`.
   - The disc's NORM bytes are signed i8: 100 % unit length, against 37–50 % read as offset-binary (`normdump.py`). The
     PS2's packet builder unpacks them the same way (`FUN_00494af8` 553869–553880: V3-8, USN clear, ROW 0x47800080 =
     65537.0, which VU1 subtracts at 0x1ee).

**Measured** (main 2c5521f's bake rendered with `pxc_pose` at the four FEL1 PCSX2 poses; masks = the render minus the
render without that model, HUD and soldier boxes cut; `measure.py`):

| model | pixels | PS2 mean sRGB | port | port − PS2 |
|---|---|---|---|---|
| `fel1_prop_foliage_d` (the pink caladium) | 210,452 | 49.5 51.6 27.9 | 50.9 51.4 26.9 | +1.4 −0.2 −1.1 |
| `fel1_prop_foliage_b` (the green bush) | 134,383 | 31.3 44.6 24.1 | 27.6 41.7 17.9 | −3.7 −2.9 −6.3 |

- The LITE patched states (`lead_tmp/re/lite/ps2exp/frames/bf2_fel1__{ctl,nosun,noamb}.png`) show the leaves are
  SUN-lit, as the port lights them: they go dark with the sun zeroed and stay lit with the ambient zeroed.
- The GS-unit light sum (card LSUM, a scratch shader) moves the caladium AWAY (+6.9 +5.3 +2.1) and barely moves foliage_b
  (−3.1 −2.1 −6.1).
- foliage_b's blue residual is [H]: the grass sprites drawn in front of it, or the HDR bloom.

**Checks** (`checks.txt`):
- `fn` on every cited line of both games.
- `word`: both vtables; 0x523014 / 0x523020 = 1; 0x522a90 = 1.0; the `Lighting` strings.
- `writers`: `DAT_00523014` / `20` (none); `DAT_00522a88` / `8c` (the key setters and the reset); `DAT_00522a90`.
- `callers`: `FUN_00597e98` (2), `FUN_0059b2f8` (2), `FUN_00493cf0` (2), `FUN_004b1688` (2).
- `absent`: `FUN_0059b2f8`, `DAT_007cd51c`.
- The ELF disassembly of both BF2 draws for bit 24, and the BF1 ↔ BF2 twin opened for every step.

**Verifier** (one Opus agent, foreground, given `claims.md` only): 17 SUPPORTED, 1 PARTLY, 0 NOT SUPPORTED. The PARTLY
is A10: its winding counts cover three models, not four, and the hellbush has one near-perpendicular vertex (367/368
per vertex, 368/368 summed). Its additions are folded above: the regions on GEO1 / END1 / BF1 RHN2, the ODF `Lighting`
bit, the signed unpack, the flag-8 pairs, and the 0x701580 queue.

**Stayed [H]:** why BF1 batches; foliage_b's blue residual; the VU prim bit 0x800800 (`re-material-flags.md` §2).

## 17. Grass display-colour compositing (2026-11-04, issue #116)

The maintainer's Dagobah comparison shows pale grass branches standing out much
more strongly in the port. A fresh local map bake from the PS2 DAG1.LVL produced
58 textures identical byte-for-byte to the existing map textures, including
`dag1_grass`; the manifest has no tier-2 texture overrides. This is evidence
against replacing or recolouring the atlas to solve this report.

BF2's grass draw `FUN_003c6e60` (current local listing 410276–410458) takes the
instance colour, scales RGB by the HDR draw factor and packs it in GS units.
The sprite builder `FUN_003c61c8` (409724–409984) likewise packs its grey and
alpha at scale 128. As elsewhere on the PS2, texture modulation and destination
blending operate on display-colour values. [V][O] Static primary evidence;
not a new independent audit of the complete grass draw state.

The port's grass pass previously alpha-blended into an sRGB attachment, which
performs the blend on linear values. It now uses the compatible display-colour
attachment view already available for water and particles. The grass shader
modulates the sampled display RGB by the sprite/patch tint, applies the existing
leaf-light and fog terms, then lets the source-over blend combine display
values. The fallback for a surface without that compatible view remains the
existing path. Grass and attached leaf patches share the pass. No new palette
or lighting constant is introduced, and no texture files are changed.

The numeric headless GPU regression covers one and two overlapping half-alpha
grass patches over a dark background, with unequal RGB tints. Each channel
must match the display-byte equation within two levels. It passed through
`scripts/gpu_tests.sh --render`, together with all 72 other renderer GPU tests.
The actual Dagobah scene test also passed: its grass changes 38,155 pixels
against the same scene without grass. Captures at (25.76, 3.3, -107), yaw -0.5,
pitch -0.25 were retained for inspection.

Checks on base `af2d62158cad6866b71c51a86a9f833a0ea234db` plus the scoped
working-tree edits: 15 water unit tests, 6 ripple unit tests, 73 serial renderer
GPU tests and 2 serial Dagobah scene tests passed; formatting and whitespace
checks passed. The release build passed. The map bake was an independent
source check; its unchanged textures need no installation into assets_baked.
Full PS2 colour/coverage parity at a matched camera, first-person water-foot
wakes, alignment acceptance and the added pass-transition cost remain
unmeasured. The maintainer identified the reported red pools as NPC mines;
mine-placement frequency is outside this visual correction.

The before/after release captures use the same CLI, camera, resolution and
scene-loading path. They differ by more than three levels on 14,155 pixels;
mean RGB over that mask changes from (39.74, 48.60, 55.11) to
(34.75, 43.62, 49.37). This measures removal of excess blend brightness, not
agreement with a PS2 screenshot at a different camera.

Final delivery: the release executable packed 24,955 current asset/mission
files, its archive footer was verified, and atomic replacement of
`dist/openbf.exe` succeeded (1,876,206,592 bytes). This supersedes the staged-only
delivery recorded for the first water-contact implementation. No command
modified the profile or playtest files under `dist/openbf_data`.


## 18. Dagobah grass lighting and terrain tone (2026-11-04, issue #116)

The new comparison exposed an incorrect assumption in sections 7 and 17:
white grass template and patch colours do not make the grass unlit. This
section supersedes that lighting claim. No palette change or map-specific
colour multiplier is needed.

Primary evidence is BF2 SLUS-21240 paused on Dagobah, read through PINE,
and a fresh local GS frame dump from the same view. All 60 live template
sprites have RGBA (1, 1, 1, 1); all 321 initialized FOLG pool entries have
white byte tints. The live class settings and the atlas RGB match the baked
PS2 assets. The emulator has no shade-boost or colour filter enabled.

The missing stream is the normal. `FUN_003c5f20` (current local listing
409632–409720) copies four packed normals from `DAT_006bf600` into the
sprite buffer. Live memory contains (0, 127, 0) for every corner.
`FUN_003c5eb0` (409616–409627) submits that buffer to `FUN_00494af8`;
`FUN_003c6e60` (410276–410458) uploads the global light set through
`FUN_0057fd90`, then draws the sprite buffer with the patch transform.
These normals reach the normal lit VU path described in
`re-model-lighting.md` section 2. The patch up vector, rather than the
billboard's camera-facing plane, determines its lighting. [V][O]

The frame dump independently confirms the result: the grass texture uses
GS MODULATE with source-over blending (ALPHA low byte 0x44), and frequently
emits RGB (80, 93, 108) in GS units where 128 is unity. Thus white inputs
are being darkened and shifted toward blue before texture modulation.
The grass TEST word is 0x5160d; exact conditional depth-write parity is
still unchecked and is not claimed by this colour correction.

The renderer now evaluates hemisphere ambient, the primary directional
light and the optional second directional light in display-colour units
for grass, using the transformed packed up normal. Static directional
lights remain included. Unlit leaf patches and the shared bolt/particle
vertex path retain their existing lighting selection.

A compatible non-sRGB view of each existing texture allows grass and
terrain to filter the authored display bytes before decoding; the texture
and its mip data are shared, not duplicated. Terrain texture layers now
also composite through the display-colour attachment. Their prior linear
source-over blend was another source of excess ground brightness. The
original layer blend evidence is `re-terrain-layers.md` section 2.

The matched-camera capture uses position (-9.229933, 6.6619954, -95.428314),
yaw 2.254181, pitch -0.372927 and vertical FOV 2*atan(1/2.047594), read from
the original view. Port captures are 640 by 448; the original screenshot
was resized to that size with box sampling for the following region
means. They measure broad colour agreement, not per-pixel registration:

| region (x0,y0,x1,y1) | PS2 mean RGB | before | corrected |
| --- | --- | --- | --- |
| left grass (95,135,270,220) | 27.74, 38.29, 39.78 | 48.74, 56.93, 50.95 | 30.30, 40.67, 41.76 |
| right grass (400,128,525,225) | 28.14, 39.09, 40.84 | 49.50, 58.58, 53.00 | 31.12, 42.14, 43.42 |
| ground (15,260,220,320) | 45.97, 57.02, 55.96 | 51.51, 64.04, 65.28 | 49.03, 61.39, 61.64 |

Source base is `af2d62158cad6866b71c51a86a9f833a0ea234db` plus the scoped
working-tree changes. Captures and raw source evidence remain local under
`engine/artifacts/dagobah-corrections/` and `engine/artifacts/lead_tmp/`.
There is still a small brightness residual, and this is not an assertion
of complete scene parity. Sprite placement randomness, mip selection,
conditional grass depth writes, BF1 visual comparisons and the cost of
additional pass transitions remain unchecked.


Final integration checks use branch revision
`1166c50b5b9161b54d7232dc4fba7bf923e0a627` plus these working-tree edits.
The intervening main merge changed vehicle baking, walker AI and exhaust
cleanup, not the grass or terrain rendering described above. The 76
serial renderer GPU checks passed; both Dagobah scene checks passed
(87,165 grass pixels changed against no grass; 11 wakes and 2,736 water
contact pixels changed). Commands were
`bash ../scripts/gpu_tests.sh --render` and
`OPENBF_WATER_CAPTURE=<local-dir> bash ../scripts/gpu_tests.sh --game water_fx::dagobah`
from `engine/`, with `cargo fmt -p game -p render --check` and
`git diff --check`. The terrain blend regression covers both ordinary and
no-discard pipeline selection. Release uses `cargo build --release -p game`.
No baked asset data changed for this runtime/shader correction.


The final release packed 24,955 current asset/mission entries into a staged
executable. The footer and archive bounds were verified, and the staged
binary rendered a 640 by 448 Dagobah frame from its embedded assets,
including the COMMON merge (+111 models, 513 draws). Atomic replacement
of `dist/openbf.exe` succeeded: 1,876,244,992 bytes. All three files under
`dist/openbf_data` matched their pre-pack SHA-256 values. The staged smoke
capture is `engine/artifacts/dagobah-corrections/packed-tone-smoke.png`.
