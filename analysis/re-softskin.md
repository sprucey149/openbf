# RE: PS2 skinned-soldier model formats (`skel` + softskin `modl`)

Scope: model side only — mesh skinning data, model skeleton, bind pose, hard-skin
attach, LOD binding. Animation keyframes (`zaa_`/`zaf_`) are another agent's;
§8 documents the contract between the two.

Tags per RE_LOG conventions: **[V]** verified from decomp/disassembly or raw
bytes, **[D]** derived (numeric check / correlation), **[H]** hypothesis.
Addresses are EE VAs (BF1 `SWBFRONT.ELF` unless marked BF2). File offsets are
into the extracted LVLs; "payload @" = first byte after the 8-byte chunk header.

Reference targets:
- BF1 `imp_inf_storm_trooper` → modl `imp_inf_stormtrooper` @0xC3928 + skel @0xC31C0
  in `extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL` (a byte-identical second
  copy lives in the next `lvl_` at 0x1DCDD0/0x1DC668).
- BF2 `rep_inf_ep3_rifleman` → modl `rep_inf_ep3trooper` @0xADFA6C + skel @0xADF308
  in `extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL`.
- BF1 `all_inf_soldier` → modl `all_inf_soldier` @0x16E488 + skel @0x16DD28 in `SIDE/ALL.LVL`.

Scratch tooling: `<scratchpad>/softskin/softskin.py` (decoder → OBJ), `scan.py`,
`census.py`, `mips.py` (mini EE disassembler), `ucfb.py`.

---

## 0. Headline findings

1. **[V] PS2 "softskin" is rigid-per-vertex.** Every soft segment in every shipped
   LVL (796 segments, 648,268 vertices: all BF1 SIDE + COMMON, all BF2 SIDE + INGAME)
   has `WGHT = (255,0,0,0)` — exactly one bone per vertex. The engine has a
   4-weight VU path but never takes it for shipped data (§4.4).
2. **[V] POSI is stored in the primary bone's bind space, not model space.**
   Dequantize into the modl vertex box, then `p_model = p_bone · M0[bone]`
   (row-vector 4×4, M0 = accumulated skel XFRM). Numeric proof: the transformed
   stormtrooper reproduces the modl vis box to 8e-6 on all six faces (§5).
3. **[V] `modl INFO[0]` = 1 is the "bone-space / rigid" flag**; it is stored at
   model+0xac and passed down the loader as the skin-mode argument (BF1
   disassembly 0x4ab348–0x4ab364; BF2 0x5909cc/0x590a08). All skinned shipped
   models have it set.
4. **[V] The 4th softskin chunk is `BMAP` (u32 count + u8 skeleton-bone indices),
   not `BONE`.** A `BONE` chunk (1 B/vertex = BMAP slot of the vertex) also exists
   but neither ELF has a case for tag 0x454E4F42 — it is ignored by the runtime.
5. **[V] `BIND` = 4×u8 per vertex = `slot*4`** (VU quadword address of the slot's
   4×4 matrix); `slot` indexes `BMAP`; `BMAP[slot]` is the skeleton bone index.
6. **[V] `skel`**: `INFO` = cstring name + u32 boneCount (variable size, 25 B for
   the stormtrooper); `NAME` = packed cstrings; `PRNT` = packed cstrings of
   *parent names* ("" for the root); `XFRM` = 12 f32 per bone = 3×3 rows + t,
   local-to-parent bind pose, row-vector convention. Bone identity everywhere
   is CRC-32 of the lowercased name (same CRC as the anim banks).
7. **[V] Hard-skin (`BNAM`) segments are also bone-space** (snowtrooper backpack on
   `bone_ribcage`, every `_low1` pelvis box) — transform by M0[bone] too.
8. ~~**[V] The class prop `GeometryLowRes` (`…_low1`) and `AnimationName` are consumed
   by neither ELF**~~ **WRONG for `GeometryLowRes` (card LOD, 2026-10-16, `re-lod.md` §5 [O]):** both ELFs read it
   (BF1 `FUN_0020f2e0` 123451, BF2 `FUN_00252bb0` 171132, printed as `0xffffffffa490eba1`, which the search
   below missed) and draw the `_low1` model for every soldier past the frame's ~4 hi-res ones. `AnimationName`
   was not re-checked. The soldier `gmod` has only `LOD0`.

---

## 1. Loader map (where the authority lives)

| what | BF1 | BF2 | notes |
|---|---|---|---|
| `lvl_` walker tag dispatch (`skel`/`modl`/`gmod`) | FUN_00175d00 (decomp l.17477–17495) | FUN_00177058 | `skel`→FUN_00498a70, `modl`→FUN_004970b8, `gmod`→FUN_001ad298 |
| skel parser | FUN_00498c08 | FUN_0057f248 | INFO/NAME/PRNT/XFRM cases |
| skel accumulate + inverse | FUN_004990c8 | FUN_0057f6b0 | called with identity @0x66ef20 / @0x73d8c0, bone 0 |
| modl parser | FUN_00497150 (via FUN_004970b8) | FUN_0057de00 | INFO/NODE/NAME/segm/SPHR/shdw |
| segm parser | **FUN_004aacd8** | **FUN_00590328** | all tag constants incl. WGHT/BIND/BMAP/BNAM |
| segment ctor / vtable select | FUN_004aa9f8 → FUN_004abde0 | FUN_005901b0 → FUN_005916b0 | MTRL flag bits pick the vtable |
| vertex packer (strip → u64 verts) | **FUN_004ab3f0** | **FUN_00590c38** | byte-identical logic |
| VIF path select | FUN_004abc70 | FUN_00591540 | 0x4000 rigid vs 0x2000 weighted |
| VIF packet builders | FUN_00398928 (rigid), FUN_00398f70 (weighted) | FUN_00494af8, FUN_00495138 | VIF UNPACK formats in §4.4 |
| model draw (per segment, matrix gather) | **FUN_00496248** | (decomp l.712834–712960) | pose = argument 3 |
| matrix gathers | FUN_00498ba8 (invBind, no pose) / FUN_00497d20 (invBind×A) / FUN_00497e90 (A raw) | FUN_0057f158 / FUN_0057e780 / FUN_0057e8e8 | selected by pose!=0 and model+0xac |
| BNAM → bone index | FUN_00496d08 (CRC lookup in skel names, 0 if missing) | FUN_0057d9f8 | |
| name CRC-32 | FUN_00444bb8 (table DAT_00521ca8) | FUN_00539420 (table DAT_0060b238) | lowercase fold `A-Z → +0x20`, init −1, final `~`, MSB-first poly 0x04C11DB7 |
| gmod parser | FUN_001ace78 | — | NAME/INFO/LOD0/LOD1/LOD2/LOWD |

The modl binds to the **most recently parsed `skel`** (global `DAT_00522abc` set in
FUN_00498a70, copied into model+0xb0 in FUN_00497150) **[V]**. In every LVL the
matching `skel` immediately precedes its `modl` (same NAME).

---

## 2. `skel` chunk **[V]**

Stormtrooper: chunk header @0xC31C0 (`73 6b 65 6c 60 07 00 00`, size 1888).

| child | payload @ | size | layout |
|---|---|---|---|
| `INFO` | 0xC31D0 | 25 | `69 6d 70 5f … 72 00` = "imp_inf_stormtrooper\0", then u32 `19 00 00 00` = **25 bones**. Variable length = strlen+1+4 (a2's "20 B" was a 15-char name). Name is FNV-hashed (FUN_00444b88) into skel+0x14; count into skel+0x10. |
| `NAME` | 0xC31F4 | 316 | 25 NUL-terminated names back-to-back, bone order. Each → CRC-32 → skel+0xc table (`FUN_00444bb8`). |
| `PRNT` | 0xC3338 | 310 | 25 NUL-terminated **parent names** (`00 62 6f 6e 65 5f 62 …` = "", "bone_b_spine", …). Resolved by CRC search over NAME; not found → parent byte 0xFF (root). Engine also fills firstChild/nextSibling bytes (skel+0x8 table, 4 B/bone: `parent, firstChild, nextSibling, ?`). |
| `XFRM` | 0xC3478 | 1200 | 25 × 48 B: `f32 r0[3], r1[3], r2[3], t[3]`. Expanded to a 64-B 4×4 with w = (0,0,0,1) at skel+0x0 table. |

**Convention [V]:** row-vector. `v_parent = v_local · R + t` where R rows are
r0,r1,r2. FUN_004990c8 computes `M0[i] = L[i] × M0[parent(i)]` top-down (VU0
`vmulabc/vmaddabc` of each L row against the parent's rows) and
`M1[i] = rigid_inverse(M0[i])` (3×3 transpose, `t' = −t·Rᵀ`) into skel+0x4.
Root parent = identity (`0x66ef20`; numerically confirmed: with identity the mesh
lands exactly in the vis box). Cross-check: the alternative order `M0 = P × L`
puts `bone_r_calf` at y = −0.16 (below the floor) instead of 0.58 **[D]**.

**XFRM is the bind (rest) pose, local to parent [V]:** the accumulated M0 places the
bone-space mesh exactly inside the modl vis box (§5). Rotation rows are unit length
(no scale) in all soldier skels checked. The bind pose is a **T-pose**: hands at
x = ±0.754, y = 1.624; head at y = 1.844.

**Bone order** is storage order; nothing depends on it — all cross-references are by
CRC (pose lookup) or by index within the same skel (BMAP, BNAM). `_low1` skeletons
use a different order (pelvis = 1).

Stormtrooper skeleton (25 bones; `local_t` = XFRM t, `model_t` = M0 translation):

| i | name | CRC-32 | parent | local_t | model_t | XFRM @ |
|---|---|---|---|---|---|---|
| 0 | DummyRoot | 0x4446E9B8 | −1 ("") | (0,0,0) | (0,0,0) | 0xC3478 (identity) |
| 1 | bone_ribcage | 0xE5B51072 | 5 bone_b_spine | (0.1592, −0.0000, −0.0000) | (0.0000, 1.4811, −0.0228) | 0xC34A8 |
| 2 | bone_r_clavicle | 0x2CD60497 | 1 bone_ribcage | (0.2351, −0.0001, 0.0000) | (0.0000, 1.7162, −0.0177) | 0xC34D8 |
| 3 | bone_r_upperarm | 0x318FF0CC | 2 bone_r_clavicle | (0.2401, 0.0001, −0.0001) | (−0.2289, 1.6490, −0.0450) | 0xC3508 |
| 4 | bone_a_spine | 0x5DE52389 | 20 bone_root | (0,0,0) | (0.0000, 1.1594, 0.0000) | 0xC3538 |
| 5 | bone_b_spine | 0x8C1C9BBA | 4 bone_a_spine | (0.1634, −0.0000, 0.0000) | (0.0000, 1.3222, −0.0142) | 0xC3568 |
| 6 | bone_l_clavicle | 0x1A757D30 | 1 | (0.2351, −0.0001, −0.0000) | (0.0000, 1.7162, −0.0177) | 0xC3598 |
| 7 | bone_l_upperarm | 0x072C896B | 6 | (0.2401, 0.0001, 0.0001) | (0.2289, 1.6490, −0.0450) | 0xC35C8 |
| 8 | bone_l_forearm | 0x8E51239C | 7 | (0.2402, −0.0000, 0.0000) | (0.4624, 1.6376, −0.0997) | 0xC35F8 |
| 9 | bone_r_forearm | 0x3DD7FD4E | 3 | (0.2402, …) | (−0.4624, 1.6376, −0.0997) | 0xC3628 |
| 10 | bone_head | 0x8AFDF5E4 | 24 bone_neck | (0.1311, …) | (0.0000, 1.8437, 0.0129) | 0xC3658 |
| 11 | bone_pelvis | 0xE6E47876 | 20 bone_root | (0,0,0) | (0.0000, 1.1594, 0.0000) | 0xC3688 |
| 12 | bone_r_thigh | 0xF6EFF696 | 11 | (−0.0943, −0.1049, 0.0091) | (−0.0943, 1.0544, 0.0091) | 0xC36B8 |
| 13 | bone_r_calf | 0xB2F01CD3 | 12 | (0.4800, 0.0001, 0.0000) | (−0.1694, 0.5806, 0.0238) | 0xC36E8 |
| 14 | bone_r_foot | 0x2F06FA86 | 13 | (0.4700, …) | (−0.2404, 0.1284, −0.0832) | 0xC3718 |
| 15 | bone_r_toe | 0x5A5BE874 | 14 | (0.1650, …) | (−0.2552, 0.0338, 0.0511) | 0xC3748 |
| 16 | bone_l_thigh | 0xC496176D | 11 | (0.0943, −0.1049, 0.0091) | (0.0943, 1.0544, 0.0091) | 0xC3778 |
| 17 | bone_l_calf | 0x87019234 | 16 | (0.4800, …) | (0.1694, 0.5806, 0.0251) | 0xC37A8 |
| 18 | bone_l_foot | 0x1AF77461 | 17 | (0.4700, …) | (0.2404, 0.1288, −0.0832) | 0xC37D8 |
| 19 | bone_l_toe | 0x2A8EADE7 | 18 | (0.1650, …) | (0.2552, 0.0341, 0.0511) | 0xC3808 |
| 20 | bone_root | 0x16E27226 | 0 DummyRoot | (0, 1.1594, 0) | (0, 1.1594, 0) | 0xC3838 (`… 5a 66 94 3f 00 00 00 00`) |
| 21 | bone_l_hand | 0xB7EC1D31 | 8 | (0.3036, …) | (0.7541, 1.6240, −0.0164) | 0xC3868 |
| 22 | bone_r_hand | 0x821D93D6 | 9 | (0.3036, …) | (−0.7541, 1.6240, −0.0164) | 0xC3898 |
| 23 | hp_weapons | 0x2B960099 | 22 bone_r_hand | (0.1662, −0.0663, 0.0069) | (−0.8962, 1.6169, 0.0923) | 0xC38C8 |
| 24 | bone_neck | 0xD2AFB28A | 1 | (0.2351, …) | (0.0000, 1.7162, −0.0177) | 0xC38F8 |

Raw XFRM[1] (bone_ribcage) @0xC34A8:
`54 3f 7f 3f d2 ec 9c 3d 00 00 00 00 | d2 ec 9c bd 54 3f 7f 3f 00 00 00 00 | 00 00 00 00 00 00 00 00 ff ff 7f 3f | 78 06 23 3e 00 08 e1 b7 7a 06 23 a5`
= r0 (0.99706, 0.07662, 0), r1 (−0.07662, 0.99706, 0), r2 (0, 0, 1), t (0.15920, −2.7e-5, −1.4e-16).
Raw XFRM[12] (bone_r_thigh) @0xC36B8: r0 (−0.1565, −0.9872, 0.0307) r1 (−0.0003, −0.0310, −0.9995) r2 (0.9877, −0.1564, 0.0046) t (−0.0943, −0.1049, 0.0091) — bone +X points down the leg.

Frame **[D]**: Y up (feet at y≈0, head 1.84); +X = character's left (`bone_l_hand` x=+0.754);
toe (z=+0.051) is in front of the foot (z=−0.083) → **+Z is forward**. Each bone's local +X
runs along the bone.

BF2 skel (`rep_inf_ep3trooper` @0xADF308) — identical layout **[V]** (FUN_0057f248 is the
same code with BF2 allocators): INFO @0xADF318 (23 B: "rep_inf_ep3trooper\0" + 25),
NAME @0xADF338 (316), PRNT @0xADF47C (310), XFRM @0xADF5BC (1200). Bone order differs
from BF1 (1 = bone_b_spine, parent 22 = bone_a_spine, model_t (0, 1.32217, −0.01415);
2 = bone_ribcage → (0, 1.48114, −0.02285); 3 = bone_l_clavicle → (0, 1.7162, −0.01769));
CRCs are the same values (same names).

---

## 3. `modl` INFO **[V]** (FUN_00497150 / BF2 FUN_0057de00)

BF1, 68 B, stormtrooper payload @0xC396C:
`01 00 00 00 | 01 00 00 00 | 00 00 00 00 | 20 eb 49 be 07 f5 52 be 8c d5 56 be | 5c db f2 3e 5d 5e 3f 3e f0 e8 56 3e | 28 f2 6d bf 12 c8 9b ba 14 75 4a be | 28 f2 6d 3f b6 9c 05 40 bb 56 5c 3e | 19 00 00 00 | 42 09 00 00`

| off | type | value | meaning |
|---|---|---|---|
| 0 | u32 | 1 | **skin-mode flag → model+0xac** (`(u32 != 0)`). 1 = vertices in bone space, rigid one-bone-per-vertex VU path, pose matrices used raw. 0 = vertices in model space, 4-weight path, matrices = invBind × pose. All 796 shipped soft segments belong to flag=1 models; all static/weapon/chunk models are 0. |
| 4 | u32 | 1 | segment count (allocates the segment pointer array) |
| 8 | u32 | 0 | stored at model+0x48; no consumer found **[H: unused]** |
| 12 | f32[3]+f32[3] | (−0.1972, −0.2060, −0.2098) .. (0.4743, 0.1869, 0.2099) | **vertex box** = POSI quantization box (bone-space extents; copied into every segment record at +0x20/+0x30 and handed to the VU as the dequant range) |
| 36 | f32[3]+f32[3] | (−0.9295, −0.0012, −0.1977) .. (0.9295, 2.0877, 0.2152) | **vis box** = model-space AABB of the bind-pose mesh (also feeds the bounding sphere at model+0x70) |
| 60 | u32 | 25 | → model+0xb4. Equals the skeleton's bone count on every model checked (60+ models, both games) **[D]** |
| 64 | u32 | 2370 | → model+0xa8. Total strip-index count over segments (== segm INFO indexCount here; same number appears in the gmod LOD0 entry) **[D]** |

BF2, 72 B, `rep_inf_ep3trooper` payload @0xADFAB8:
`01 00 00 00 | 00 00 00 00 | 01 00 00 00 | 00 00 00 00 | boxes… | 19 00 00 00 | 8a 0a 00 00`
= u32 flag → +0xac (byte), **u32 flag2 → +0xad (byte; 0 on every model seen; consumed by
the material setup FUN_00597e98 as its 5th arg)**, u32 segCount, u32 read-and-discarded,
12 f32 (vertex box (−0.2027,−0.2095,−0.2099)..(0.5042,0.1856,0.2099), vis box
(−0.9295,−0.0012,−0.2331)..(0.9295,2.0827,0.2035)), u32 25, u32 2698. So the "extra
leading u32" of the existing Rust parser (`base = 16`) is right for the boxes; the
flag is still word 0 in both games.

Other modl children: `NAME` (FNV-hashed → modl registry table 0x69c678), `NODE`
("DummyRoot"; parser strips a trailing `_NN` from `hp_…` names, CRC result unused),
`segm`×n, `SPHR` (16 B: center xyz + radius, e.g. (0, 1.0433, 0.0087) r=1.2494 @0xCC42C),
`shdw` (shadow volume, separate class), BF2 adds `VRTX` (4 B, @0xADFA98, unknown **[H]**).

---

## 4. Softskin segment payloads **[V]**

Stormtrooper `segm` header @0xC39B0 (payload 0xC39B8, 35436 B). Children in file order:

| child | payload @ | size | content |
|---|---|---|---|
| `INFO` | 0xC39C0 | 8 | `6b 05 00 00 42 09 00 00` → vertexCount 1387, indexCount 2370 |
| `MTRL` | 0xC39D0 | 4 | `01 00 00 00` flags (BF2: 25 B — u32 flags, u32 diffuse, u32, u32, 2×u8, cstring) |
| `RTYP` | 0xC39DC | 7 | "Normal" (BF2: u32 = 0 @0xADFB44) |
| `TNAM` | 0xC39EC | 20 | u32 slot 0 + "Imp_inf_trooper" (case as stored) |
| `_PS2` | 0xC3A08 | 4 | 0x1110 — parsed into the temp record word 0, never read by packer/VIF builder (**[V] unused on this path**; value unexplained **[H]**) |
| `STRP` | 0xC3A14 | 4740 | 2370 × u16: `71 83 0d 83 8c 03 62 03 8f 03 …` |
| `POSI` | 0xC4CA0 | 8322 | 1387 × 3×u16 (bone space, quantized into the vertex box) |
| `NORM` | 0xC6D2C | 4161 | 1387 × 3×i8 (bone space, ÷127) |
| `TEX0` | 0xC7D78 | 5548 | 1387 × 2×i16 (÷2048) |
| `BMAP` | 0xC932C | 25 | `15 00 00 00` + `01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f 10 11 12 13 15 16` → u32 count 21, then u8 **skeleton bone indices** per slot. Slot j ↔ VU matrix j. (Excludes DummyRoot, bone_root, hp_weapons, bone_neck.) |
| `BONE` | 0xC9350 | 1387 | 1 × u8 per vertex: `0e 0e 12 12 0d 0d 11 11 …` = **BMAP slot** of the vertex (== `BIND[v][0] >> 2` for 648,268/648,268 vertices). **Not parsed by either ELF** (no 0x454E4F42 case in FUN_004aacd8 / FUN_00590328). |
| `WGHT` | 0xC98C4 | 5548 | 1387 × 4×u8 weights, 255 = 1.0: `ff 00 00 00` for every vertex |
| `BIND` | 0xCAE78 | 5548 | 1387 × 4×u8: `38 34 34 34 | 38 34 34 34 | 48 44 44 44 …` = per weight the **matrix quadword address = slot × 4**; bytes 1..3 pad with a neighbouring slot when the weight is 0. All 4 bytes are multiples of 4 and `< 4·len(BMAP)` in every segment (census). |

Loader facts (FUN_004aacd8): `WGHT` and `BIND` are read as `vertexCount` u32 each;
`BMAP` as u32 count + count bytes; `BNAM` as a string resolved through FUN_00496d08
(CRC search in the bound skel, **0 if absent**); `TNAM` slots ≥ 4 are dropped; `COL0`
sets MTRL bit 0x8000. The 8 floats at model+0x50..0x6c (vertex box) are copied into
the record handed to the packer.

### 4.1 Runtime vertex packing (FUN_004ab3f0 / BF2 FUN_00590c38) **[V]**

For each strip index k (in `STRP` order — vertices are re-fetched per index, no index
buffer reaches the VU):
```
idx      = STRP[k] & 0x7FFF
restart  = STRP[k] >> 15                 // bit 15 set
if restart: parity = even                // reset winding parity
flags    = (restart ? 0x8000 : 0) | (parity odd ? 0x4000 : 0)
if (WGHT && BIND && skinflag)            // skinflag = model INFO[0]
     w16 = flags | ((BIND[idx][0] >> 2) << 2)   // = flags | (BIND0 & 0xFC)  → matrix address in bits 2..7
else w16 = flags
packed[k] = u16 POSI.x | POSI.y<<16 | POSI.z<<32 | w16<<48
parity ^= 1
```
Then NORM (3 B), TEX0 (4 B), COL0 (4 B; `& 0xFFFFFF` when MTRL bit 0x40), and — only when
both exist — WGHT/BIND (4 B each) are gathered into per-index arrays in the same order.

### 4.2 Strip / winding semantics **[V]**
Each index with bit 15 set is a "no primitive ends here" vertex (GS ADC-style) and
resets the parity; both first indices of a strip carry it (`71 83 0d 83 8c 03 …`).
The existing Rust `unstitch_strips` (restart on a flagged *pair*, alternate winding)
is equivalent on shipped data. **Winding check [V]:** with triangles emitted as
`(a,b,c)` for even t and `(b,a,c)` for odd t, `cross(b−a, c−a)·(n_a+n_b+n_c) > 0`
for 1627/1627 stormtrooper triangles (BF2 ep3trooper 1722/1722; snowtrooper
1495/1496; low1 305/308). I.e. the emitted order is **CCW as seen from the stored
normal**. If the renderer mirrors an axis to change handedness, flip winding too.

### 4.3 VIF path selection (FUN_004abc70; disassembly 0x4abc70–0x4abd7c) **[V]**
```
if (!hasBMAP)                → FUN_00398928(ctx, n, pos, NORM, TEX0, 0, COL0)                     // static
else if (skinflag != 0)      → mask 0x4000; FUN_00398928(ctx, n, pos, NORM, TEX0, 0, COL0)        // RIGID: bone in pos.w
else                         → mask 0x2000; FUN_00398f70(ctx, n, pos, NORM, TEX0, 0, 0, WGHT, BIND) // 4-weight
```
The 9th argument (BIND) is the stack slot `sw v0,0x0(sp)` @0x4abd3c with `v0 = lw 0x10(s1)`
— Ghidra dropped it. `skinflag` is `lw a1,0xac(a0)` @0x4ab358 → `sw a1,0(sp)` @0x4ab364
in FUN_004aacd8, forwarded by FUN_004aa9f8 (`lw a0,0xb0(sp)`/`sw a0,0(sp)` @0x4aac68/0x4aac78).
BF2: `lbu a1,0xac(s4)` @0x5909cc → `sw a1,0x10(sp)` @0x590a08 → `argument 12` of FUN_005901b0
→ FUN_005916b0 → FUN_00590c38/FUN_00591540 with the same branches (BF2's two other stack
args are the BMAP count/pointer, `s6/s7` @0x5905e8/0x5905fc).

### 4.4 VIF UNPACK formats (FUN_00398928 / FUN_00398f70) **[V]**
Per batch of ≤ 60 vertices (`0xB7 / streams & ~3`, strips overlapped by 2), header
qword then: pos `0x6D` = **V4-16 unsigned** (x,y,z,w=flags|addr); TEX0 `0x75` = V2-16
signed; NORM `0x7A` = V3-8 signed; COL0 `0x6E` = V4-8 unsigned; WGHT `0x6E` V4-8 unsigned;
BIND `0x6E` V4-8 unsigned (weighted path only, which also drops COL0). Dequant box goes
up separately (FUN_0039bd98(ctx, box.min, box.max)). Dequantization = `min + q·(max−min)/65535`
(`/65535` matches the vis box to 8e-6, `/65536` to 1.5e-5; q spans exactly 0..65535) **[D]**.

### 4.5 Matrices handed to the VU (FUN_00496248, per segment) **[V]**
```
if (seg.bmapCount == 0)                       // static or BNAM hard-skin
    pose == 0 : draw(seg, World)
    pose != 0 : draw(seg, A[seg.boneIdx] × World)             // boneIdx from BNAM (0 if unknown)
else                                          // BMAP present
    pose == 0            : M[j] = invBind[BMAP[j]]             // FUN_00498ba8 (skel+4 table) — wrong for bone-space data; unreachable for soldiers
    pose && flag == 0    : M[j] = invBind[BMAP[j]] × A[BMAP[j]] // FUN_00497d20 — model-space 4-weight path (dead for shipped data)
    pose && flag != 0    : M[j] = A[BMAP[j]]                    // FUN_00497e90 — RAW pose matrix (what soldiers use)
    draw(seg, World, M, bmapCount)
```
`A[b]` = pose matrix for bone b (§8); missing bone → identity. BF2 identical
(FUN_0057f158 / FUN_0057e780 / FUN_0057e8e8 branch on the model's byte at +0xac, decomp l.712934).

---

## 5. Vertex space — settled **[V]**

Hypothesis test on the stormtrooper: dequantize POSI into the vertex box, transform each
vertex by `M0[BMAP[BIND0>>2]]` (bone→model bind matrix, row-vector), take the AABB:

| | x | y | z |
|---|---|---|---|
| transformed extents | −0.9294691 .. 0.9294680 | −0.0011865 .. 2.0876850 | −0.1977135 .. 0.2151696 |
| modl vis box | −0.9294763 .. 0.9294763 | −0.0011885 .. 2.0876899 | −0.1977122 .. 0.2151746 |

max |Δ| = 8.2e-6 (quantization noise). Untransformed POSI cannot exceed the 0.67 × 0.39 × 0.42 m
vertex box, so model-space storage is excluded. The same 0.0000 match holds for every
soldier in IMP/ALL/REP (`scan.py`: stormtrooper, shocktrooper, scout, atstpilot,
darktrooper, snowtrooper, marksperson, aaylasecura, ep3trooper, all `_low1`s), and the
`_low1` hard segments only match when transformed by their BNAM bone (err_raw 1.9 m vs
0.000). Vehicles (xwing, fightertank, arc170 …) show 1–7 m residuals in *both* hypotheses
— their vis box evidently spans animated poses; not a soldier concern **[H]**.

Consequences for the renderer:
```
p_model  = dequant(POSI[v]) · A[bone]          bone = BMAP[BIND[v][0] >> 2]; bind pose A = M0
n_model  = (NORM[v]/127)   · R(A[bone])        (3×3 part; no scale in XFRM)
world    = p_model · World
```
Weights can be ignored (all 255/0/0/0). If you want the format-general form anyway:
flag=1 (shipped): `p = Σ_j (WGHT_j/255) · (p_bone · A[BMAP[BIND_j>>2]])` (what the 0x2000 VU
path would do; identical to the above for single weights). flag=0 (never shipped):
`p = Σ_j w_j · (p_modelspace · invBind[b_j] · A[b_j])`.

Three decoded vertices (segment @0xC39B0, vertex box above):

| v | POSI @ / raw | q | bone-space | BIND @ / bytes | slot → bone | WGHT | BONE | NORM | TEX0 (raw → uv) | model-space |
|---|---|---|---|---|---|---|---|---|---|---|
| 0 | 0xC4CA0 `58 63 3a 9d 39 b4` | (25432, 40250, 46137) | (0.0634, 0.0353, 0.0857) | 0xCAE78 `38 34 34 34` | 14 → 15 bone_r_toe | `ff 00 00 00` @0xC98C4 | 0x0e | (10, 89, 89) | (830, 716) → (0.4053, 0.3496) | (−0.1770, −0.0012, 0.1237) |
| 1 | 0xC4CA6 `58 63 3a 9d 39 b4` | same | same | 0xCAE7C `38 34 34 34` | 14 → 15 | same | 0x0e | (10, 89, 89) | (598, 663) → (0.2920, 0.3237) | same (UV seam duplicate) |
| 2 | 0xC4CAC `5b 63 2d 9d a0 4b` | (25435, 40237, 19360) | (0.0634, 0.0352, −0.0858) | 0xCAE80 `48 44 44 44` | 18 → 19 bone_l_toe | `ff 00 00 00` | 0x12 | (10, 89, −89) | (598, 663) | (0.1769, −0.0010, 0.1237) |
| 150 | 0xC5024 | (62480, 35947, 42972) | (0.4430, 0.0095, 0.0654) | 0xCB0D0 `2c 28 28 28` | 11 → 12 bone_r_thigh | `ff 00 00 00` @0xC9B1C | 0x0b | (82, −10, 96) | (436, 2035) | (−0.0990, 0.6066, 0.0135) (knee) |

---

## 6. Hard-skin (`BNAM`) segments on soldiers **[V]**

Present on: every `_low1` soldier (`bone_pelvis`, 24 verts — e.g. `imp_inf_stormtrooper_low1`
segm @0xCC968, BNAM payload @0xCCB38 = "bone_pelvis\0"), `imp_inf_snowtrooper` (backpack,
`bone_ribcage`, 52 verts, segm @0x162BCC), `all_inf_soldier` uses a *soft* single-bone segment
for its packs instead (BMAP = [bone_ribcage], 183 verts). Weapons (`imp_weap_inf_rifle` etc.)
are separate flag=0 models whose single BNAM bone sits at identity.

Attach: the segment is drawn with `A[boneIdx] × World` where `boneIdx = index of CRC(BNAM)`
in the bound skel (FUN_00496d08; **0 = DummyRoot if the name is missing**, no error). Its
POSI/NORM are in that bone's bind space and share the model-level vertex box — for the
snowtrooper the soft+hard union reproduces the vis box only when the backpack is
transformed by `M0[bone_ribcage]` (err 0.000 vs 0.070 without). So: **treat a BNAM segment
exactly like a soft segment whose every vertex has slot → that bone.** (`_PS2` 0x28 / 0x6C on
these segments.) Hard segments have no BMAP/BONE/WGHT/BIND and no TNAM on the pelvis box.

Hardpoints (`hp_weapons`, child of `bone_r_hand`) are ordinary skel bones; FUN_00496d78 /
FUN_00496f18 fetch a bone's matrix by CRC for attaching weapons **[D]**.

---

## 7. LOD / low-res / class binding

- entc `imp_inf_storm_trooper` (IMP.LVL @0xEBC78; TYPE @0xEBC98) PROPs (FNV-1a|0x20 keys) **[V]**:
  `GeometryName` (0x47c86b4a) = "imp_inf_stormtrooper"; `GeometryLowRes` (0xa490eba1) =
  "imp_inf_stormtrooper_low1"; `FirstPerson` (0x6363d774) = "IMP\impstrp;imp_1st_stormtrooper";
  `AnimationName` (0x98557a5e) = "all_inf_snowtrooper" (sic). No `SkeletonName`.
  BF2 `rep_inf_ep3_rifleman`: GeometryName "rep_inf_ep3trooper", GeometryLowRes
  "rep_inf_ep3trooper_low1", FirstPerson "REP\reptroop;rep_1st_trooper".
- **Consumers [V]:** `GeometryName` hash hits 4 class parsers in each ELF (BF1 decomp l.67335,
  75496, 81687, 89813). Each calls the common binder `FUN_001e3678(obj, 0x47c86b4a, name)` and
  then FNV-looks the name up in table 0x566bc8 = the `coll` registry (filled by the coll loader
  FUN_00164e28) to attach the "CollisionMesh"; render-model resolution goes through the gmod
  registry 0x5818c8 (lookups at l.60653 / 65471, FUN_001ad4d0) → LOD0 modl name → modl registry
  0x69c678 **[D]**.
  `GeometryLowRes` and `AnimationName`: **zero hits in either decomp (hex or decimal forms)**.
  **CORRECTION (card LOD, 2026-10-16):** `GeometryLowRes` has one hit per decomp in the 64-bit form
  `0xffffffffa490eba1` (BF1 123451 `FUN_0020f2e0`, BF2 171132 `FUN_00252bb0`) and one `lui`/`ori` build per ELF;
  it builds the soldier class's two-model LOD record (`re-lod.md` §5).
  `SkeletonName` (0x7012f6cd) / `SkeletonLowRes` (0x55f297d2) are looked up in the *animation*
  registry 0x6ff2d8 (BF1 l.123351 / 123128), i.e. they select anim banks, not `skel` chunks.
  No "_low1" / "%s_low" string exists in either ELF.
- `gmod` (FUN_001ace78) **[V]**: `NAME` (FNV → registry 0x5818c8), `INFO` = u32 + f32,
  `LOD0`/`LOD1`/`LOD2`/`LOWD` = cstring modl name (FNV → modl registry 0x69c678) + u32.
  Stormtrooper gmod @0xCC43C: INFO `02 00 00 00 | 00 00 80 3f` (2, 1.0), LOD0 =
  "imp_inf_stormtrooper" + 0x942 (2370 = its index total). `_low1` gmod @0xCEF60: INFO
  (0, 1.0), LOD0 = "imp_inf_stormtrooper_low1" + 0x1FC (508). Missing LOD0 falls back to
  LOD1/LOD2. Names starting "coll_" get a merged bbox (FUN_001a91e0). INFO u32/f32
  semantics **[H]** (2 vs 0 — hi-res vs low-res marker? f32 = LOD scale 1.0?).
  Vehicles use `<name>LOWD` models via `LOWD`; **soldiers have LOD0 only**.
- Therefore **[D]**: the class renders `GeometryName` (modl `imp_inf_stormtrooper`, skel of the
  same name bound by load order). The `_low1` model (14-bone skel, 367+24 verts) is munged in
  with its own gmod/skel/modl but no engine path referencing it was found (open question #3).

---

## 8. Contract with the animation rig **[V] mechanism, [H] spaces**

- Model bone identity = **CRC-32(lowercase name)** (same routine family as the anim-bank
  hash: BF1 FUN_00444bb8 / BF2 FUN_00539420). CRCs above are computed with
  `crc32_swbf()` in `softskin.py` and match the engine's table algorithm.
- Pose object (BF1: `pose+0x408` → skel, `pose+8` → 0x100-bucket hashtable): FUN_00497f28
  fills it from a rig whose per-bone records are 0x70 B with the CRC at +0x60 and 64-B
  matrices at `rig+0x10 + j·0x40`; FUN_00497b80(pose, boneIdx) looks up `skel.crc[boneIdx]`
  and returns the matrix or the identity (0x66ef20 / BF2 0x73d8c0).
- The VU receives `A[b]` **raw** for flag=1 models (§4.5). For the bind-pose sanity check the
  anim agent's evaluator, at the rig's rest pose, must reproduce `M0[b]` (the table in §2)
  for the 21 BMAP bones — that is the acceptance test that the rig and model skeleton share
  frames. BMAP omits DummyRoot, bone_root, bone_neck, hp_weapons on the stormtrooper; the
  model's `bone_neck`/`bone_root` still must exist in the rig for the hierarchy to match.
- `AnimationName = "all_inf_snowtrooper"` on the stormtrooper is dead data on PS2 (no
  consumer); BF1 uses the global `human_animation_*` banks (see re-anim-banks.md).

---

## 9. Reference decoder

`<scratchpad>/softskin/softskin.py <LVL> <modl name> [out.obj]` — parses the `skel`
(prefers the one with the model's NAME, else the last preceding one), the `modl`
(68/72-B INFO auto-detected), dequantizes POSI, transforms bone-space vertices (soft:
`BMAP[BIND0>>2]`, weighted-blend general form; hard: BNAM bone) and normals by M0,
unstitches strips with the engine's per-index rule, and writes OBJ (`vt` written as
`1−v` — orientation unverified). Prints skeleton, BMAP, weight histograms, BIND/BONE
consistency, and extents.

| model | segs | verts (soft+hard) | tris | bones | extents x / y / z (m) | OBJ |
|---|---|---|---|---|---|---|
| BF1 `imp_inf_stormtrooper` | 1 | 1387 | 1630 | 25 | ±0.929 / −0.001..2.088 / −0.198..0.215 | `softskin/stormtrooper.obj` |
| BF1 `imp_inf_stormtrooper_low1` | 2 | 367 + 24 | 314 | 14 | ±0.908 / −0.001..2.087 / −0.191..0.169 | `softskin/stormtrooper_low1.obj` |
| BF1 `all_inf_soldier` | 3 (face/packs/body) | 113+183+988 | 1651 | 25 | ±0.929 / 0.000..2.048 / −0.396..0.216 | `softskin/all_inf_soldier.obj` |
| BF2 `rep_inf_ep3trooper` | 1 | 1710 | 1722 | 25 | ±0.929 / −0.001..2.083 / −0.233..0.204 | `softskin/rep_ep3trooper.obj` |

Up axis **+Y** (2.09 m tall, feet on y≈0, T-pose arms spanning 1.86 m).

Census (`census.py`, all BF1 SIDE/*.LVL + COMMON.LVL, all BF2 SIDE/*.LVL + INGAME.LVL):
796 soft segments, 648,268 vertices — `multiweight = 0`, weight sums all 255, 0 BIND bytes
not ≡ 0 mod 4 or ≥ 4·len(BMAP), `BONE == BIND0>>2` for 100%, `INFO[0] == 1` for 100% of
models carrying soft segments; every soft segment has both BMAP and BONE.

---

## 10. Tier-1 test vectors (for `mungefs` unit tests)

BF1 `extracted/bf1/lvl/DATA/_LVL_PS2/SIDE/IMP.LVL` (offsets = payload):
1. `skel` @0xC31C0 (header `73 6b 65 6c 60 07 00 00`): INFO @0xC31D0 → name "imp_inf_stormtrooper", **count 25** (`19 00 00 00` @0xC31E5).
2. NAME @0xC31F4: bone[0] "DummyRoot", bone[1] "bone_ribcage", bone[20] "bone_root", bone[23] "hp_weapons", bone[24] "bone_neck".
3. PRNT @0xC3338: parent[0] = "" → −1; parent[1] = "bone_b_spine" → 5; parent[20] = "DummyRoot" → 0; parent[12] = "bone_pelvis" → 11.
4. XFRM @0xC3478: bone[20] `bone_root` @0xC3838 = identity rows, t = (0, 1.1593735, 0) (`5a 66 94 3f`); bone[1] @0xC34A8 t = (0.1592044, −2.68e-5, −1.4e-16), r0 = (0.9970601, 0.0766236, 0); bone[12] @0xC36B8 t = (−0.0942804, −0.1049374, 0.0091177).
5. Accumulated model_t: bone_ribcage (0, 1.4811, −0.0228); bone_r_calf (−0.1694, 0.5806, 0.0238); bone_l_hand (0.7541, 1.6240, −0.0164); bone_head (0, 1.8437, 0.0129) (tolerance 1e-3).
6. `modl` @0xC3928: INFO @0xC396C: u32 (1, 1, 0); vertex box (−0.197186, −0.206013, −0.209799)..(0.474330, 0.186883, 0.209873); vis box (−0.929476, −0.001189, −0.197712)..(0.929476, 2.087690, 0.215175); tail u32 (25, 2370).
7. `segm` INFO @0xC39C0 → (1387, 2370); BMAP @0xC932C → count 21, bytes `01..13,15,16` (skel indices 1–19, 21, 22); BONE @0xC9350 first bytes `0e 0e 12 12`; WGHT @0xC98C4 `ff 00 00 00`; BIND @0xCAE78 `38 34 34 34`, +8 `48 44 44 44`; STRP @0xC3A14 `71 83 0d 83 8c 03` → 0x371 (flag), 0x30d (flag), 0x38c.
8. Vertices 0 / 2 / 150 as in §5 table (bone-space and model-space, tolerance 1e-3).
9. Transformed-mesh AABB == vis box within 1e-4 on all six components; 1630 non-degenerate triangles; winding test §4.2 = 1627/0.
10. `imp_inf_stormtrooper_low1` @0xCC8DC: skel @0xCC498 has 14 bones (INFO 30 B), segm[0] @0xCC968 is hard (BNAM "bone_pelvis" @0xCCB38, 24 verts), segm[1] BMAP = 11 slots [1..11]; union AABB == its vis box (−0.908, −0.001, −0.191)..(0.908, 2.087, 0.169).

BF2 `extracted/bf2/lvl/DATA/_LVL_PS2/SIDE/REP.LVL`:
11. `skel` @0xADF308: INFO @0xADF318 "rep_inf_ep3trooper" + 25; bone[1] = bone_b_spine parent 22 (bone_a_spine), XFRM @0xADF5EC t = (0.16342, −1e-5, 0), model_t (0, 1.32217, −0.01415); bone[3] bone_l_clavicle model_t (0, 1.7162, −0.01769).
12. `modl` @0xADFA6C: INFO @0xADFAB8 (72 B) u32 (1, 0, 1, 0), vertex box (−0.20265, −0.20946, −0.20987)..(0.50420, 0.18560, 0.20987), tail (25, 2698); segm INFO @0xADFB10 (1710, 2698); BBOX @0xADFBA0 == vertex box; BMAP @0xAE67D8 = 21 slots `01..0a,0c..16`; POSI @0xAE10E8 `8c 5d 94 9e 32 b2` → q (23948, 40596, 45618) → bone (0.0556, 0.0353, 0.0823), BIND @0xAE8974 `3c 3c 3c 3c` → slot 15 → bone 17 bone_r_toe → model (−0.1795, −0.0012, 0.1156); v2 @0xAE10F4 `8e 5d 87 9e b4 4d` → slot 19 → bone 21 bone_l_toe → (0.1794, −0.0010, 0.1156). AABB == vis box (±0.92948, −0.00120..2.08267, −0.23314..0.20353).

---

## 11. Open questions, ranked by risk to a correct render

1. **Pose-matrix space from the anim evaluator** — must be bone→model in the model skel's
   frames (rest pose == M0). If the rig evaluates to something else (e.g. parent-relative or
   with a different root), the rigid skin explodes. Acceptance test in §8. **[H]**
2. **Handedness / facing.** +Z forward is inferred from toe-vs-foot only; UV V-flip unverified
   (`vt` written as 1−v). Verify against a texture + an animation with known facing. **[D/H]**
3. **Low-res soldier model.** ~~`GeometryLowRes` is unconsumed~~ — answered by card LOD (`re-lod.md` §5 [O]): the
   class's LOD record holds the hi and `_low1` models; the soldier queue's frame budget gives ~4 soldiers the hi
   model (BF1 nearest first; BF2 largest on screen, and only within ~10 radii) and draws the rest `_low1`.
4. **BNAM name miss → bone 0** (DummyRoot) silently; a strict parser should error instead. **[V]**
5. **modl INFO extras**: BF1 word 2 / BF2 word 3 unused; BF2 `+0xad` second flag (0 everywhere)
   feeds material setup; `VRTX` (BF2, 4 B) and `_PS2` values unexplained; harmless. **[H]**
6. **MNAM** (material name, hashed) appears on some segments (`all_inf_soldier` body) — ignore. **[V]**
7. Static (pose-less) draw of a flag=1 model gathers inverse-bind matrices → would be wrong;
   never happens for soldiers (pose is always passed). **[V]**

---

## 12. Summary for the lead

1. Parse `skel`: INFO = cstring + u32 N; NAME/PRNT = N packed cstrings (PRNT are *names*, "" = root); XFRM = N×12 f32 (rows r0,r1,r2 + t), local-to-parent bind pose, **row-vector** math.
2. `M0[i] = L[i] × M0[parent]` (root parent = identity) = bind bone→model; keep `inv(M0)` only for the never-used flag=0 path.
3. `modl INFO[0]` (both games) = 1 means POSI/NORM are in **bone space**; BF2 INFO is 72 B with the boxes at +16; last two u32 = bone count, index total.
4. Soft segment = STRP/POSI/NORM/TEX0 + `BMAP` (u32 n + n×u8 skel bone idx) + `BONE` (u8 slot/vertex, ignore) + `WGHT` (4×u8) + `BIND` (4×u8 = slot·4).
5. Every shipped vertex has one weight: `bone = BMAP[BIND[v][0] >> 2]`; `p_model = dequant(POSI) · M0[bone]`, `n = NORM/127 · R(M0[bone])`.
6. Hard-skin `BNAM` segments: same, with `bone = index of CRC32(BNAM)` in the skel; soldiers use them for backpacks and the `_low1` pelvis box.
7. Dequant `min + q·(max−min)/65535` with the modl vertex box (same box for all segments); triangle order from `unstitch_strips` is CCW w.r.t. the normals.
8. Skinned render at runtime: per BMAP slot upload `A[bone]` (animated bone→model, rest = M0) × World; bones are matched to the rig by CRC-32(lowercase name), not by index.
9. The class renders `GeometryName` (and `GeometryLowRes` past the soldier queue's budget — corrected by card LOD, `re-lod.md` §5); gmod LOD0 just names the same modl (+ index count).
10. Decoder + OBJs + vectors are in `<scratchpad>/softskin/`; stormtrooper = 1387 verts / 1630 tris / 25 bones, 2.09 m tall, Y up, T-pose, matches the vis box to 1e-5.
