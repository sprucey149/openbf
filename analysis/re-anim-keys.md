# RE: Soldier animation keys — `zaa_` TNJA/TADA decode + `zaf_` rig

Closes `re-anim-banks.md` open item #4 (TNJA/TADA internals) and adds the
`zaf_` rig so a clip can be played on a skinned soldier. Companion to the
model-side (softskin `WGHT/BIND/BONE` + `skel`) note being written in parallel.

Tags: **[V]** verified from decomp + raw bytes, **[D]** derived (bytes + tests,
no single decomp line proves it), **[H]** hypothesis. Addresses are EE VAs in
`analysis/SWBFRONT.ELF-decomp.c` (BF1) / `analysis/SWBFRNT2.ELF-decomp.c` (BF2).
ELF VA→file: `off = va − 0x160000 + 0x1000` (both games, LOAD phdr).
Reference decoder + scratch scripts:
`<scratch>`
(`animkeys.py` decoder, `zaa_parse.py`, `zaf_dump.py`, `zerocrc.py`,
`validate_all.py`, `match_bones.py`, `clip_names2.py` → `clip_list.txt`).

---

## 0. Summary (details below)

- A clip = per-track streams, one track per bone, bone identified by the
  **CRC-32 of the bone name** (same hash as clip names). Rig bones use the same
  CRC, so anim ↔ rig ↔ model `skel` join by `crc32(lowercase(name))`. **[V]**
- Rotation = 4 × i16 streams (x,y,z,w) ÷ 2047 (engine constant
  `0.0004885198`), **absolute local-to-parent quaternion**, one key per frame.
  Translation = 3 × i16 streams, `base[3] + scale × v`, only for tracks whose
  bit is set in the MINA u64 mask; otherwise constant `base` (= rest offset). **[V]**
- Stream byte code: `i16 first`, then per frame `0x80 nn` = hold nn frames,
  `0x81 lo hi` = absolute i16, else `i8` delta. Verified on all 39 422 streams
  of the 15 human banks (every stream ends exactly where the next begins). **[V]**
- Runtime sampling: frame `f = trunc(t·(F−1))`, slerp (shortest path) to
  frame `f+1`, lerp translation; `t` advances by `dt·30/F` (BF1) or
  `dt·30/(F−1)` (BF2). Loop clips duplicate frame 0 at frame F−1. **[V]**
- Root motion lives in the `DummyRoot` track (z forward, metres); BF1 soldier
  code differences it per tick and turns it into velocity. Strip it for
  in-place playback. **[D]**

---

## 1. Runtime objects (what the loaders build) **[V]**

### 1.1 Global anim registry (both `zaa_` and `zaf_` register here)
`zaa_` loader `FUN_004be900`, `zaf_` loader `FUN_004bec78`: on `NAME` they
CRC the string (`FUN_00444b88`→`FUN_00444bb8`) and look it up in the global
hash table at `0x6ff2d8` (`…
if absent they allocate a **0x20-byte entry** (`FUN_004be8b0`) and insert it
(`FUN_004449f8`). So the `zaa_` bank and the `zaf_` rig that share a NAME
(e.g. `human_animation_0`) land in the **same** entry:

| entry off | set by | meaning |
|---|---|---|
| +0x00 | loaders | name CRC |
| +0x08 | `FUN_004bedf8` (zaf_ BIN_) | `skel` sub-object pointer (= rig+0x10) |
| +0x0c | `FUN_004bedf8` | rig object pointer |
| +0x10, +0x14 | `FUN_004beab0` (zaa_ BIN_) | bank object pointer (stored twice) |
| +0x18 | `FUN_004beae0` (EVNT) | event table (human banks ship no EVNT) |

Hash-table probe (`FUN_00444998`/`FUN_004449f8`, used everywhere): keys in
`tbl[0..n)`, values in `tbl[n..2n)`, `n = sizeArg>>1`; start bucket
`key & (n−1)`, on mismatch step to `idx−1` (wrap to `n−1`), stop at key 0.

### 1.2 Bank object (`FUN_004cdf28` + `FUN_0043d528/0043d590/00446c20/00447420`)
```
bank+0x004  u32  animCount (incremented per record)
bank+0x008  hash: 256 keys (0x400 B) + 256 values (0x400 B)   key = clip CRC → anim record*
bank+0x808  ptr  first anim record
bank+0x80c  u16  animCount
bank+0x810  anim records, 0xC B each:  {+0 track records*, +4 crc, +8 u16 frames, +0xA u16 tracks}
            track records, 0x38 B each (see §3), animCount×0x10 unused bytes,
            then a verbatim copy of the TADA payload (BIN_ size − 0x18 − MINA/TNJA sizes)
```
`FUN_00446c20(rec, minaEntry, tnja, trackRecBase, tadaBase)` builds one clip:
`rec+4=crc`, `rec+8=frames`, `rec+0xA=tracks`, then for track i
`FUN_00447420(trackRec_i, rec, tnja_i, tadaBase, (mask>>i)&1)`.
**The u64 mask bit i means "track i has translation streams".**

### 1.3 Rig object (`FUN_004ce068`→`FUN_004ce720` size, `FUN_004ce238` fill, `FUN_00441538` SKEL)
```
rig+0x00 ptr skel (= rig+0x10) ; rig+0x04 ptr JSET slots (0x32 B each) ; rig+0x08 u8 jsetCount
skel+0x00 ptr bones ; skel+0x04 u32 boneCount ; skel+0x0c u32 inserted
skel+0x10 hash: 64 keys + 64 values (0x200 B)  key = bone CRC → boneIndex+1   (max 64 bones)
skel+0x210 boneCount×16 B (unknown), bones start at (0x210+16n+0xF)&~0xF
bone (0x70 B): +0x00 4×4 f32 matrix (bind/world, built by FUN_004419c0 via FUN_00446110(quat,pos))
               +0x40 f32[4] rest quat xyzw   +0x50 f32[3] rest pos   +0x5c ?   +0x60 u32 crc
               +0x64 u8 parentIdx  +0x65 u8 firstChildIdx  +0x66 u8 nextSiblingIdx (0xFF = none)  +0x67 u8 =1
```
`FUN_00441538` swaps the (single) parentless joint to **index 0**, resolves the
three link CRCs to byte indices with `FUN_00441960` (hash lookup −1, 0xFF for
CRC 0), and `FUN_004419c0(rig, 0)` walks child/sibling links composing
`world = local × parent` (row-vector VU0 form; = `parent∘local` column form).

### 1.4 Binding a clip to a rig (`FUN_00446d90`, BF2 `FUN_0053b900`) **[V]**
For each track i of the clip: `boneIdx = hash(skel+0x10, trackRec_i.crc) − 1`;
`ctx+0x900+2i = boneIdx`, `ctx+0x940+2·boneIdx = i` (0xFFFF = unbound).
Bones that have no track in the clip simply have no key source (left to the
other blend layer / rest pose by the caller — not traced, see §9).

---

## 2. `zaf_` BIN_ = rig ("ZAFF" tree) **[V]**

Reversed-ASCII tags (file bytes `FFAZ` = tag `ZAFF`; compare `FUN_00447e30`
which packs the ELF string big-endian: strings at ELF 0x55e218 `ZAFF MTLS JSTS
SKNS JSET VERS SIZE`, 0x557708 `SKEL JONT`). Standard `[tag][u32 size][payload]`,
4-aligned. Layout (BF1 `human_animation_0` zaf_ @0x27B8D8, BIN_ payload @0x27B904,
size 0xC4EA; BF2 INGAME.LVL `human_0` @0x178E50 is byte-for-byte the same rig):

```
ZAFF {
  VERS  u16 8, u16 0                       (FUN_004ce5f0 requires 8/0)
  SIZE  u32[9]: matlCount, jsetCount, skinCount, boneCount, 0,0,0,0,0   (human: 8,1,8,31)
  MTLS { MATL × matlCount }                (0x5C or 0x1C B; RGBA + flags + texture name, e.g. "ALL_pilot_HOTH.tga")
  JSTS { JSET × jsetCount }                (FUN_004ce510: u32 n1, n1×u32 joint idx; u32 n2, n2×u32 idx; n ≤ 0x20)
  SKNS { SKIN × skinCount }                (u32 material, u32 vertCount, u32 15, packed PS2 skin verts)
  SKEL { JONT × boneCount }                (0x2C B each, below)
  PXYS (0 B)  SHDS (0 B)
}
JONT (0x2C): +0x00 u32 nameCrc | +0x04 f32[4] rest quat (x,y,z,w) | +0x14 f32[3] rest pos (local, metres)
             +0x20 u32 parentCrc (0 = root) | +0x24 u32 firstChildCrc | +0x28 u32 nextSiblingCrc
```
The MTLS/JSTS/SKNS payloads are the reference model the rig was munged from
(materials + skinned verts); the engine only *skips* them at load
(`FUN_004ce238` iterates and discards; JSET is kept). Not needed for playback. **[V/H]**

Rest pose composes as `world = parent ∘ local(quat, pos)`; FK of the JONT
table puts `bone_head` at y=1.844, feet at y≈0.13, toes 0.03 — a standing
figure with bones running along local **+X** (calf 0.48 m, foot 0.47 m). **[D]**

### 2.1 The `human` rig (31 joints, BF1 `human_animation_0` = BF2 `human_0`) **[V bytes / D names]**
Names resolved by CRC-matching bone names harvested from all LVL `skel`
chunks + ELF strings (CRC is case-insensitive; shown lowercase). File offsets
are JONT payload starts in BF1 COMMON.LVL.

| # | JONT @ | CRC | name | parent | rest pos (x,y,z) | rest quat (x,y,z,w) |
|--:|---|---|---|---|---|---|
| 0 | 0x287798 | 4446E9B8 | DummyRoot | — | 0,0,0 | 0,0,0,1 |
| 1 | 0x2877CC | B2F01CD3 | bone_r_calf | bone_r_thigh | 0.4800,0,0 | 0.0003,−0.0001,0.1308,0.9914 |
| 2 | 0x287800 | 2F06FA86 | bone_r_foot | bone_r_calf | 0.4700,−0.0002,−0.0001 | 0.0767,−0.0545,−0.5540,0.8272 |
| 3 | 0x287834 | 5A5BE874 | bone_r_toe | bone_r_foot | 0.1650,−0.0001,0 | 0.0002,−0.0002,−0.3033,0.9529 |
| 4 | 0x287868 | 5DE52389 | bone_a_spine | bone_root | 0,0,0 | 0.4779,0.5212,0.4779,0.5212 |
| 5 | 0x28789C | 8C1C9BBA | bone_b_spine | bone_a_spine | 0.1634,0,0 | 0,0,0.0161,0.9999 |
| 6 | 0x2878D0 | E5B51072 | bone_ribcage | bone_b_spine | 0.1592,0,0 | 0,0,0.0383,0.9993 |
| 7 | 0x287904 | 1A757D30 | bone_l_clavicle | bone_ribcage | 0.2351,−0.0001,0 | −0.5983,0.0539,−0.7991,0.0267 |
| 8 | 0x287938 | 072C896B | bone_l_upperarm | bone_l_clavicle | 0.2401,0.0001,0.0001 | −0.0038,−0.1182,0.0557,0.9914 |
| 9 | 0x28796C | 8E51239C | bone_l_forearm | bone_l_upperarm | 0.2402,0,−0.0001 | 0,0,−0.2512,0.9679 |
| 10 | 0x2879A0 | B7EC1D31 | bone_l_hand | bone_l_forearm | 0.3036,0.0001,−0.0001 | 0.0102,−0.0214,0.0024,0.9997 |
| 11 | 0x2879D4 | C496176D | bone_l_thigh | bone_pelvis | 0.0943,−0.1049,0.0091 | −0.5461,0.4518,−0.4665,0.5292 |
| 12 | 0x287A08 | 2CD60497 | bone_r_clavicle | bone_ribcage | 0.2351,−0.0001,0 | 0.5983,−0.0539,−0.7991,0.0267 |
| 13 | 0x287A3C | 318FF0CC | bone_r_upperarm | bone_r_clavicle | 0.2401,0.0001,−0.0001 | 0.0038,0.1182,0.0557,0.9914 |
| 14 | 0x287A70 | 3DD7FD4E | bone_r_forearm | bone_r_upperarm | 0.2402,0,0.0001 | 0,0,−0.2512,0.9679 |
| 15 | 0x287AA4 | 821D93D6 | bone_r_hand | bone_r_forearm | 0.3036,0.0001,0.0001 | −0.0087,0.0217,0.0024,0.9997 |
| 16 | 0x287AD8 | 87019234 | bone_l_calf | bone_l_thigh | 0.4800,0,0 | −0.0003,0.0001,0.1308,0.9914 |
| 17 | 0x287B0C | 1AF77461 | bone_l_foot | bone_l_calf | 0.4700,−0.0002,0.0001 | −0.0767,0.0545,−0.5540,0.8272 |
| 18 | 0x287B40 | D2AFB28A | bone_neck | bone_ribcage | 0.2351,−0.0001,0 | 0,0,0.1064,0.9943 |
| 19 | 0x287B74 | 8AFDF5E4 | bone_head | bone_neck | 0.1311,0,0 | 0,0,−0.1175,0.9931 |
| 20 | 0x287BA8 | 2A8EADE7 | bone_l_toe | bone_l_foot | 0.1650,−0.0001,0 | −0.0002,0.0002,−0.3033,0.9529 |
| 21 | 0x287BDC | E6E47876 | bone_pelvis | bone_root | 0,−0.0001,0 | 0,0,0,1 |
| 22 | 0x287C10 | F6EFF696 | bone_r_thigh | bone_pelvis | −0.0943,−0.1049,0.0091 | −0.4665,0.5292,−0.5461,0.4518 |
| 23 | 0x287C44 | C7C743D8 | *(unresolved, identity helper)* | DummyRoot | 0,0,0 | 0,0,0,1 |
| 24 | 0x287C78 | B4209B59 | *(unresolved, identity helper)* | DummyRoot | 0,0,0 | 0,0,0,1 |
| 25 | 0x287CAC | 16E27226 | bone_root | B4209B59 | 0,**1.1594**,0 | 0,0,0,1 |
| 26 | 0x287CE0 | 2FE55C87 | *(unresolved)* | bone_l_hand | −0.7217,−0.2194,−1.6242 | 0.7005,0.0962,0.0962,0.7005 |
| 27 | 0x287D14 | 2B960099 | hp_weapons | bone_r_hand | 0.1662,−0.0663,0.0069 | −0.7356,0.0927,−0.6680,0.0631 |
| 28 | 0x287D48 | 1A14D260 | *(unresolved)* | bone_r_hand | −0.7217,−0.2194,1.6242 | 0.0962,−0.7005,0.7005,−0.0962 |
| 29 | 0x287D7C | D1E1F308 | ALL_Pilot_Helmet | bone_head | −1.8436,−0.0059,0 | −0.5011,−0.4988,−0.5011,0.4988 |
| 30 | 0x287DB0 | 7238CCAE | *(unresolved, identity helper)* | DummyRoot | 0,0,0 | 0,0,0,1 |

Hierarchy: `DummyRoot → B4209B59 → bone_root → {bone_a_spine → bone_b_spine →
bone_ribcage → {l/r clavicle → upperarm → forearm → hand (→ hp_weapons on
the right)}, bone_neck → bone_head}, bone_pelvis → {l/r thigh → calf → foot →
toe}}`. The 5 unresolved joints are never animated (see §4.4) and can be
ignored; `ALL_Pilot_Helmet` is a stray reference-model bone (tracked in only
two prone clips). Other rigs: `human_animation_fp`/`humanfp` (8 joints:
DummyRoot, bone_shoulders, l/r forearm, l/r wrist, hp_weapons, hp_camera),
`human_animation_lowrez`/`humanlz` (14 joints, no bone_root/spine —
`bone_pelvis` is the moving root there). 136/148 joint CRCs across every
`zaf_` in both games resolve to names.

### 2.2 Join with the model `skel` **[V]**
BF1 `SIDE/IMP.LVL` `skel` @0xC31C0 (`imp_inf_stormtrooper`, 25 bones; INFO/NAME/
PRNT/XFRM as in a2) carries the **same names** (`DummyRoot, bone_root,
bone_pelvis, bone_a_spine … hp_weapons`) with the same parents (only
difference: model `bone_root` parents directly to `DummyRoot`; the rig
inserts the identity helper `B4209B59`). For every bone, `XFRM.pos == JONT.pos`
(4 decimals) and `XFRM 3×3 == R(JONT.quat)ᵀ`, i.e. the model stores the same
local-to-parent rotation in row-vector layout. Therefore: **hash the model's
bone name with the same CRC, look it up in the rig, and drive it with the
track of the same CRC.** Model bones absent from the rig/clip keep their rest.

### 2.3 Which rig a soldier uses **[D]**
> 2026-10-16 (card USKN, `re-soldier-skeleton-skins.md` §1-§2 [O]): the clips play on the human rig, but a class that
> names a `SkeletonName` is POSED on that rig - its body bones keep that rig's rest translations, and bone_root's clip
> translation is scaled by `SkeletonRootScale`.

Soldier class init `FUN_00214060` CRCs a name (the class's animation-bank
name) → registry entry stored at `class+0x1698`; the soldier ctor
(`FUN_002128c0`, decomp line ~125057) builds its skeleton instance from
`entry+8` (the `skel` of the `zaf_` registered under that NAME). The prop
player `FUN_004b55e8` (line ~575780) searches the five banks
`human_animation_0..4` for the clip and falls back to the **first** bank's
rig when the bank that has the clip carries no `zaf_` — only
`human_animation_0` (BF1) / `human_0` (BF2) ship a rig, so all soldier clips
are played on that 31-joint rig.

---

## 3. TNJA record = one track, 0x30 bytes **[V]**
Copied to the runtime 0x38 track record by `FUN_00447420` (BF2: identical
layout read by `FUN_00531e00/FUN_00532070`). LibSWBF2 `TNJA.cpp` agrees.

| off | type | field | runtime (0x38 rec) |
|---|---|---|---|
| +0x00 | u32 | **boneCrc** — CRC-32 of bone name | +0x00 |
| +0x04 | u32[4] | **rotOff[x,y,z,w]** — byte offset of each rotation stream in TADA | +0x04 pointers `tada+off` |
| +0x14 | u32[3] | **transOff[x,y,z]** — byte offset of each translation stream (0 if none) | +0x14 pointers, or 0 when mask bit clear |
| +0x20 | f32[3] | **transBase** — translation bias; for untranslated tracks = rest offset | +0x20 |
| +0x2c | f32 | **transScale** — translation quantum (0 for untranslated tracks) | +0x2c |
| — | — | (mask bit i of the MINA entry) | +0x30 anim record*, +0x34 hasTranslation |

Tracks of a clip are consecutive in TNJA (clip k's first track index = Σ
bones of clips 0..k−1). No flags, no key counts: every stream has exactly
`frames` values (§4). Full-body clips list tracks in this order: `DummyRoot,
C7C743D8, bone_root, bone_a_spine, bone_b_spine, bone_ribcage, bone_l_clavicle,
bone_l_upperarm, bone_l_forearm, bone_l_hand, bone_r_clavicle, bone_r_upperarm,
bone_r_forearm, bone_r_hand, hp_weapons, bone_neck, bone_head, bone_pelvis,
bone_r_thigh, bone_r_calf, bone_r_foot, bone_r_toe, bone_l_thigh, bone_l_calf,
bone_l_foot, bone_l_toe` (26). Upper-body clips have 16 (no DummyRoot/pelvis/
legs) or 17 (+DummyRoot) tracks; two prone clips add `ALL_Pilot_Helmet` (27).

Raw example (BF1 COMMON.LVL, `human_rifle_diveforward`, TNJA rec #2 = bone_root @ **0x204064**):
```
26 72 e2 16  2f 2d 00 00  33 2d 00 00  37 2d 00 00  3b 2d 00 00
3f 2d 00 00  67 2d 00 00  bd 2d 00 00  1a cc 8a bd  6c ab ff 3e
30 7b 28 bd  cd 44 67 39
crc=16E27226 rot=[11567,11571,11575,11579] trans=[11583,11623,11709]
base=(-0.067772, 0.499355, -0.041133) scale=2.20555e-4
```

---

## 4. TADA encoding **[V]**

TADA payload = bytes after the `TADA` chunk header up to the end of BIN_
(the `TADA` size field is 1 byte off from `SMNA+0x10`; the engine ignores it
and copies `BIN_size − 0x18 − MINA − TNJA` bytes; LibSWBF2 notes the same).
Offsets in TNJA are relative to the payload start (BF1 `human_animation_2`:
payload @ **0x21204C**).

### 4.1 One stream = one scalar channel, one value per frame
Decoder = BF1 `FUN_004474b8` (rotation, 4 channels) / `FUN_00447770`
(translation, 3 channels); BF2 `FUN_00531e00` / `FUN_00532070` identical.

```
i16  v0                      value at frame 0
then, until `frames` values are produced:
  0x80 nn      hold: the current value repeats for nn more frames (nn = u8)
  0x81 lo hi   absolute: value = i16 (lo|hi<<8), one frame
  bb           delta: value = i16_wrap(value + (int8)bb), one frame   (bb ∉ {0x80,0x81})
```
Rotation: `q = (v_x, v_y, v_z, v_w) × 0.0004885198` (float constant in both
ELFs; = 1/2047, identity key is `(0,0,0,2047)`). Quantised **unit
quaternion, xyzw, local-to-parent, absolute** (it replaces the rest rotation;
§6.2). Translation: `p = base + scale × (v_x, v_y, v_z)` (metres, local). A
track without the mask bit has no translation streams: `p = base` for every
frame (the runtime stores null stream pointers and `+0x34 = 0`).

Each channel is decoded independently; the runtime keeps a per-track state
(`0x28 B` rot / `0x20 B` trans: current frame, 4/3 cursors, current values,
previous-frame values, per-channel remaining-hold bytes) and decodes forward
incrementally; asking for `frame < state.frame` resets to frame 0 and
re-decodes; asking for `state.frame − 1` returns the cached previous values
(this is how the f/f+1 pair is served cheaply). Linear re-decoding from the
stream start (as in `animkeys.py` and LibSWBF2's `AnimDecompressor`) gives
identical values.

### 4.2 Frame indexing, interpolation, timing
`FUN_0043da00(t, ctx, out, track)` (BF2 `FUN_00532598`), `t ∈ [0,1]`:
```
f     = (uint)( (float)(F−1) · t )           // truncation, F = MINA frames
q0,p0 = sample(track, f)
if ctx.interpolate (BF1 ctx+0x1340, default 1; BF2 byte +0x1335):
  n = f+1
  if n ≥ F:  looping ? (n = 1, p1 = p(1) + p(F−1)) : (n = F−1)     // wrap rule
  q1,p1 = sample(track, n)
  frac  = (F−1)·t − trunc((F−1)·t)
  q     = slerp(q0, q1, frac)   // FUN_00445720 / BF2 FUN_0053a3e8: dot<0 ⇒ negate q1 (shortest path);
                                // 1−|dot| < 1e-4 ⇒ lerp; acosf + table sin; renormalise if | |q|²−1 | > 1e-4
  p     = p0 + (p1 − p0)·frac
out = {q (16 B), p (16 B, w garbage)}
```
Time advance `FUN_0043d898(dt, ctx)` (BF2 `FUN_00532440`):
`t += dt · fps / F` in **BF1** but `t += dt · fps / (F−1)` in **BF2**, with
`fps = ctx+0x1320 = 30.0` (every caller passes `0x41f00000`). Loop (BF1
ctx+0x1334 / BF2 byte +0x1332): `t −= 1` while `t > 1`; non-loop: clamp to
1 and set the finished flag (+0x133c / +0x1334). Reverse flag (+0x1338 /
+0x1333) subtracts instead. Anim-set layer (`FUN_00230f98`): `t = seconds ·
rate · 30/F`. Consequences: BF1 plays the F−1 intervals of a clip over
`F/30 s` (effective `30·(F−1)/F` fps, e.g. 28.4 fps for the 19-frame run),
BF2 plays them at exactly 30 fps over `(F−1)/30 s` — matching the duration
formulas already in `re-anim-banks.md`. **[V]**

Loop clips duplicate frame 0 at frame F−1 (run/walk/idle: max 3.5° between
the two, vs 12–19° between frames 0 and 1) — consistent with the wrap rule
jumping to frame **1**. Because `t` wraps before reaching 1.0, the `f = F−1`
branch is only hit at `t == 1.0` exactly (frac 0), so the odd `p(1)+p(F−1)`
term never has weight in practice. **[D]**

Stateless single-frame samplers exist too (`FUN_00447a50/FUN_00447a78`:
state frame = 0xFFFF forces a reset).

### 4.3 Mask semantics recap
`MINA.boneFlagMask` bit i ⇔ track i (within the clip, i < 64) has 3
translation streams. Across the human banks the translated tracks are
`bone_root` (height/bob), `DummyRoot` (locomotion displacement), `hp_weapons`
(weapon drop in death clips, mask 0x4004), and in the fp bank the forearms/
shoulders; every other track has `transOff = 0`, `scale = 0`, `base = rest
offset` (e.g. bone_b_spine base 0.16341 = JONT pos 0.1634). **[V]**

### 4.4 Whole-bank validation (`validate_all.py`) **[V]**
All 15 human banks (8 BF1 + 7 BF2), 39 422 streams: **0 streams end at the
wrong byte** (each decode consumes exactly up to the next stream's offset /
end of TADA). `C7C743D8` and `DummyRoot` never rotate (max 1−|w| = 0).
Quaternion norms are 1 ± 0.04 except **four degenerate keys** in the shipped
BF1 data (also in BF2's copies): `human_rifle_prone_turnleft` bone_a_spine
f1 = (0,0,0,0); anim_4 clip #8 bone_a_spine f25 = (−4,0,−4,0); anim_4 clip
#29 bone_l_thigh f2 = (0,0,0,0); `human_grenade_stand_shoot2`
bone_l_clavicle f54 (|q| = 0.25). Each sits between a key and its negation
(q → 0 → −q): the munger lerped through a sign flip. The runtime's slerp
renormalises the neighbour (and would produce NaN only if sampled at exactly
frac 0 on the zero key). **Port rule: if |q| < ε use the neighbouring key.**
Sign flips between consecutive frames (q, −q) do occur; shortest-path
interpolation handles them.

---

## 5. Root motion **[D]**
- `DummyRoot` (rig index 0 after the root swap; track 0 of full-body clips)
  carries locomotion: `human_rifle_stand_runforward` (19 fr) moves z 0 → 4.014
  m (= 6.69 m/s at 30 fps), `stand_walkforward` (21 fr) 0 → 2.581 m (3.87 m/s),
  `diveforward` (35 fr) 0 → 5.404 m; `bone_root` translation stays ≈ (0.04,
  1.00, −0.01) in locomotion clips (height only). Upper-body clips (16
  tracks) have no DummyRoot track (track 0 = the identity helper) ⇒ zero
  motion.
- BF1 soldier class init `FUN_0021f178` (decomp line ~134179) stores
  `class+0x2760 = rigIndex(crc("DummyRoot"))` (string at ELF 0x5338e8); the
  per-tick soldier update `FUN_00218fa0` (lines ~129801–129990) samples that track via
  `FUN_0043dc18`, differences its **z** translation against the stored
  previous value, passes Δz to `FUN_0021ac10(Δz, entity)` and sets the
  velocity fields `+0x17c/+0x17e = (Δ/dt)·facing`. `FUN_0043dd88`
  (BF2 `FUN_00532858`) is the generic "root delta since last sample" helper
  with wrap handling `(p(1.0) − prev) + (cur − p(0.0))`.
- The ELF also hard-codes `crc("bone_root") = 0x16E27226` for rig lookups
  (lines 113897, 116151, 130281).
- For openbf: play clips in place by zeroing the DummyRoot translation (or
  consume Δz as root motion); bone_root keeps its animated height.

---

## 6. Reference decoder and sanity checks

`python3 animkeys.py <LVL> <bank> <clip|crc:XXXXXXXX|idx:N> [--rig human_animation_0] [--frames 0,10] [--csv out.csv] [--all]`
prints the MINA/TNJA offsets, every track's TNJA fields, per-frame raw i16 +
float quat/translation, the DummyRoot/bone_root trajectories, a unit-norm /
per-frame-step check, and an FK summary (feet/head/hands world positions)
computed both with `q` and with `q*` so the convention is testable.

### 6.1 Results
| clip (bank, MINA #) | frames | check |
|---|---|---|
| `human_rifle_diveforward` (anim_2 #9 @0x20113C) | 35 | max ‖q‖ err 0.003; FK(q): f0 feet y 0.11/0.15, head 1.17; f10 head 0.58 (mid-roll); f34 feet 0.13, head 1.28; FK(q*): feet above head ⇒ **`q` as-is, `world = parent∘local`** |
| `human_rifle_stand_idle_emote` (anim_0 #31 @0x1C5244, crc C1D44B82) | 71 | ‖q‖ err 3e-4, max step 4.0°; FK f0: root (0.05,1.02,−0.11), head (−0.07,1.61,0.08), feet y 0.13 — standing |
| `human_rifle_stand_runforward` (anim_2 #28 @0x20126C, crc 0F0F4E11) | 19 | DummyRoot z +4.014, root height 1.00; max step 42° (foot) |
| `human_rifle_stand_walkforward` (anim_2 #33 @0x2012BC, crc 9AE2ABBD) | 21 | DummyRoot z +2.581 |
| BF2 INGAME `human_1` #0 `diveforward` @0x1ACEEC | 35 | decodes **identical** to BF1 (all 26 tracks, all frames); TNJA crc/base/scale identical, only stream offsets differ |

### 6.2 Absolute vs delta rotation
Idle frame 0 vs rig rest: clavicles 5°/15° from rest but 179°/171° from
identity; hp_weapons 1.5° from rest / 173° from identity; toes 0° from rest.
Keys are absolute local rotations, not deltas on the rest pose. **[D]**

---

## 7. Tier-1 test vectors (BF1 `COMMON/ENG/COMMON.LVL`, bank `human_animation_2`)

Bank: `zaa_` @0x201060, BIN_ payload @0x20108C (`SMNA`, ver 8, 1430 tracks,
61151 TADA bytes, 55 anims), MINA @0x2010A4 (+8 = entries), TNJA @0x20141C
(+8 = records, 1430×0x30), TADA @0x212044 (+8 = payload @0x21204C).

**MINA entry #9 @0x20113C** (`human_rifle_diveforward`, crc `DC2308EB` =
crc32lower("human_rifle_diveforward")):
`05 00 00 00 00 00 00 00 eb 08 23 dc 23 00 1a 00` → mask 0x5 (tracks 0,2
translated), frames 35, tracks 26; first track index = 234 (9 earlier clips × 26) → TNJA rec @0x20141C+8+234·0x30 = **0x204004**.

**TNJA rec 0 @0x204004** (DummyRoot):
`b8 e9 46 44 | bb 2c 00 00 bf 2c 00 00 c3 2c 00 00 c7 2c 00 00 | cb 2c 00 00 cf 2c 00 00 d3 2c 00 00 | 00 00 00 00 00 00 00 00 4c f0 2c 40 | ed 05 ad 3a`
→ crc 4446E9B8, rot [11451,11455,11459,11463], trans [11467,11471,11475], base (0,0,2.70217), scale 1.32006e-3.
**TNJA rec 2 @0x204064** (bone_root): see §3. **TNJA rec 8 @0x204184** (bone_l_forearm):
`9c 23 51 8e | 83 31 00 00 87 31 00 00 8b 31 00 00 c1 31 00 00 | 00×12 | e8 ec 75 3e 00 00 00 00 00 00 00 a4 | 00 00 00 00`
→ crc 8E51239C, rot [12675,12679,12683,12737], no translation, base (0.24016, 0, −1.1e-16), scale 0.

Streams (payload-relative offset → file offset):
- DummyRoot rot x @11451 (0x214D07): `00 00 80 22` → 0 for all 35 frames (34 = 0x22 holds); rot w @11463: `ff 07 80 22` → 2047 (=1.0).
- DummyRoot trans z @11475 (0x214D1F): `01 f8 81 92 f8 81 28 f9 …` → f0 −2047 → z = 2.70217 + 1.32006e-3·(−2047) ≈ 0.0000; f10 −521 → 2.01441; f34 → 5.40433.
- bone_root trans y @11623 (0x214DB3): `8f 03 81 06 03 81 cc 03 81 8e 04 81 43 05 81 e5 05 81 6f 06 68 3a 18 e4 81 80 06 …` → [911, 774, 972, 1166, 1347, 1509, 1647, 1751, 1809, 1833, 1805, …] → y(f0) = 0.499355 + 2.20555e-4·911 = **0.70028**, y(f10) = **0.89746**.
- bone_l_forearm rot z @12683 (0x2151D7): `65 f9 e2 e2 1a 7a 81 44 fa 7a 81 7b fb 81 62 fc 37 81 de fb …` → [−1691, −1721, −1751, −1725, −1603, −1468, −1346, −1157, −926, −871, −1058, …]; rot w @12737 (0x21520D): `82 04 d3 cf 2a 81 fa 04 81 92 05 74 81 99 06 81 22 07 1a 9c …` → [1154, 1109, 1060, 1102, 1274, 1426, 1542, 1689, 1826, 1852, 1752, …].

Expected decoded keys (raw i16 → float; translation in metres):

| track / bone | frame 0 | frame 10 |
|---|---|---|
| 0 DummyRoot | q raw (0,0,0,2047) = (0,0,0,1); t raw (0,0,−2047) = (0, 0, 0.0000) | q identity; t raw (0,0,−521) = (0, 0, 2.01441) |
| 2 bone_root | q identity; t raw (409, 911, −170) = (0.02243, 0.70028, −0.07863) | q identity; t raw (165, 1805, −274) = (−0.03138, 0.89746, −0.10157) |
| 8 bone_l_forearm | q raw (0,0,−1691,1154) = (0, 0, −0.82609, 0.56375); t = base (0.24016, 0, 0) | q raw (0,0,−1058,1752) = (0, 0, −0.51685, 0.85589) |
| 16 bone_head | q raw (890,−375,−850,1593) = (0.43478, −0.18319, −0.41524, 0.77821); t = (0.13113, 0, 0) | q raw (688,43,−1080,1597) = (0.33610, 0.02101, −0.52760, 0.78017) |
| 23 bone_l_calf | q raw (0,0,1660,1197) = (0, 0, 0.81094, 0.58476); t = (0.48, 0, 0) | q raw (0,0,477,1991) = (0, 0, 0.23302, 0.97264) |

Frame 34 (last): DummyRoot t (0, 0, 5.40433); bone_root t (0.02155, 0.89018, −0.09561);
bone_l_forearm q (0, 0, −0.59111, 0.80655). FK world positions (q, parent∘local,
rig §2.1): f0 bone_root (0.02, 0.70, −0.08), bone_head (−0.25, 1.17, 0.17),
bone_l_foot (0.25, 0.11, 0.20), bone_r_foot (−0.18, 0.15, −0.36); f10 bone_root
(−0.03, 0.90, 1.91), bone_head (0.03, 0.58, 2.50).

Rig vectors (BF1 COMMON.LVL, JONT payloads): bone_root @0x287CAC
`26 72 e2 16 | 00 00 00 80 ×3 00 00 80 3f | 00 00 00 00 5a 66 94 3f 00 00 00 00 | 59 9b 20 b4 | 89 23 e5 5d | 00 00 00 00`
(pos y 1.1594, parent B4209B59, child bone_a_spine); bone_pelvis @0x287BDC
(`76 78 e4 e6`, quat identity, pos (0, −1.14e-4, 0), parent 16E27226, child
F6EFF696); bone_l_calf @0x287AD8 (`34 92 01 87`, quat (−3.0e-4, 1.1e-4,
0.13078, 0.99141), pos (0.48000, 8.4e-6, −3.6e-5), parent C496176D, child 1AF77461).
Rig header @0x27B904: `FFAZ e0 c4 00 00 | SREV 04 00 00 00 08 00 00 00 | EZIS 24 00 00 00 08 00 00 00 01 00 00 00 08 00 00 00 1f 00 00 00 …`.

CRC constants: `crc("DummyRoot")=4446E9B8`, `crc("bone_root")=16E27226`,
`crc("bone_pelvis")=E6E47876`, `crc("hp_weapons")=2B960099`,
`crc("human_rifle_stand_idle_emote")=C1D44B82`, `crc("human_rifle_stand_runforward")=0F0F4E11`.

---

## 8. Clip inventory (BF1, `human_rifle_*`) **[V frames / D names]**
Names recovered by CRC brute force over the ELF vocabulary (the BF2 special-
anim table at 0x5fde48 lists the stance/action words: `stand crouch prone
standalert crouchalert pronealert idle_emote idle_checkweapon idle_lookaround
turnleft turnright walkforward runforward walkbackward runbackward walkright
runright walkleft runleft idle_takeknee shoot shoot2 shoot_secondary(2) charge
reload …`; BF1 0x517af0 has the thrown_*/getup/death set). 210 of the 362
distinct clip CRCs in the 15 human banks resolve; the BF1 rifle set:

| bank | # | clip | frames | tracks | mask |
|---|--:|---|--:|--:|---|
| anim_0 | 26 | human_rifle_crouch_idle_emote | 71 | 26 | 0 |
| anim_0 | 27 | human_rifle_crouch_reload | 75 | 26 | 0x4 |
| anim_0 | 28 | human_rifle_jetpack_hover | 61 | 26 | 0x4 |
| anim_0 | 29 | human_rifle_prone_idle_emote | 51 | 27 | 0 |
| anim_0 | 30 | human_rifle_prone_reload | 75 | 26 | 0 |
| anim_0 | 31 | **human_rifle_stand_idle_emote** | 71 | 26 | 0 |
| anim_0 | 32 | human_rifle_stand_reload | 75 | 26 | 0x4 |
| anim_2 | 0 | human_rifle_crouch_getdown_prone | 40 | 26 | 0x4 |
| anim_2 | 1–3 | human_rifle_crouch_hit{front,left,right} | 1 | 26 | 0 |
| anim_2 | 4 | human_rifle_crouch_shoot | 16 | 26 | 0 |
| anim_2 | 5–6 | human_rifle_crouch_turn{left,right} | 10 | 26 | 0x4 |
| anim_2 | 7–8 | human_rifle_crouch_walk{backward,forward} | 28 | 26 | 0x5 |
| anim_2 | 9 | **human_rifle_diveforward** | 35 | 26 | 0x5 |
| anim_2 | 10 | human_rifle_jump | 1 | 26 | 0 |
| anim_2 | 11–12 | human_rifle_land{hard,soft} | 34 / 17 | 26 | 0x4 |
| anim_2 | 13–15 | human_rifle_prone_hit{front,left,right} | 1 | 26 | 0 |
| anim_2 | 16–17 | human_rifle_prone_turn{left,right} | 15 | 26 | 0x4 |
| anim_2 | 18–21 | human_rifle_prone_walk{backward,forward,left,right} | 32/32/27/27 | 26 | 0x5 |
| anim_2 | 22–25 | human_rifle_stand_hit{back,front,left,right} | 1 | 26 | 0 |
| anim_2 | 27 | human_rifle_stand_runbackward | 25 | 26 | 0x5 |
| anim_2 | 28 | **human_rifle_stand_runforward** | 19 | 26 | 0x5 |
| anim_2 | 29 | human_rifle_stand_shoot | 16 | 26 | 0x4 |
| anim_2 | 30–31 | human_rifle_stand_turn{left,right} | 10 | 26 | 0x4 |
| anim_2 | 32 | human_rifle_stand_walkbackward | 32 | 26 | 0x5 |
| anim_2 | 33 | **human_rifle_stand_walkforward** | 21 | 26 | 0x5 |
| anim_2 | 36–52 | human_rifle_thrown_* (bounce/flying/land/rest, 1-frame poses; `thrown_flail` 26 fr) | 1 | 26 | 0 |
| anim_2 | 53–54 | human_rifle_thrown_tumble{back,front} | 33 | 26 | 0x5 |
| anim_4 | 7 | human_rifle_die | 60 | 26 | 0x4004 |
| anim_4 | 9 | human_rifle_fall | 1 | 26 | 0 |
| anim_4 | 10–11 | human_rifle_prone_getup_{crouch,stand} | 44 / 45 | 26 | 0x4 |
| anim_4 | 12 | human_rifle_prone_shoot | 5 | 27 | 0 |
| anim_4 | 13–16 | human_rifle_stand_death_{backward,forward,left,right} | 35/40/31/35 | 26 | 0x4004 |
| anim_4 | 17 | human_rifle_stand_getdown_prone | 40 | 26 | 0x4 |
| anim_4 | 18–19 | human_rifle_stand_getup{back,front} | 53 / 58 | 26 | 0x4 |
| alert | 3 | human_rifle_stand_idle_emote_alert | 16 | 26 | 0 |
| alert | 4 | human_rifle_stand_runforward_alert | 19 | 26 | 0x5 |

(anim_0 #17 = 55 fr/17 tracks/0x5 and a few others remain unresolved; the full
per-bank list incl. BF2 `human_0..4` — which use `_full`/`_lower` suffixes,
e.g. `human_rifle_stand_idle_emote_full` — is in `clip_list.txt`.) Bank
`human_animation_1` = grenade/pistol-ish sets, `_3` mostly unresolved (tool/
melee), `_fp` = first-person (8–9-track clips on the fp rig, names unresolved).

---

## 9. Open questions, ranked by playback risk
(2026-10-05: item 1 is ANSWERED in `re-soldier-anim-layers.md` - two clip players, an upper-body bone mask, the
per-weapon sets. Erratum for 1.1: `FUN_00444b88` is the FNV wrapper (`FUN_00444b30`), so the registry at 0x6ff2d8
is keyed by the FNV of the NAME; clip and bone names are the CRC.)
1. **Blend layers** (upper-body 16/17-track clips vs full-body 26): which
   layer wins per bone and what unbound bones show (rest pose vs the other
   layer) lives in the anim-set giants (BF1 `FUN_0022d828`, BF2
   `FUN_00282f78`, untraced). Single-clip playback is unaffected. [H]
2. **Root motion consumption**: BF1 turns DummyRoot Δz into entity velocity
   (§5); how the playback rate is tied to ODF MaxSpeed was not traced.
   Playing in place needs DummyRoot translation stripped. [D]
3. **Matrix handedness**: the engine's quat→matrix (`FUN_00446110`, VU0
   macro soup) was not decoded symbolically; the convention (q as-is, v' =
   q v q⁻¹, `world = parent∘local`) rests on FK tests (feet on ground, head
   1.6 m, both games) and on the model `skel` XFRM = R(q)ᵀ identity. A
   mirrored result in the Rust port means the row/column convention got
   swapped, not the data. [D]
4. **BF1 timing quirk**: `t += dt·30/F` (not F−1) — a BF1 clip is ~F/(F−1)
   slower than 30 fps; BF2 is exact. Port per game. [V]
5. **Degenerate zero keys** (4 in BF1 data) must be guarded. [V]
6. Unresolved names: 5 rig helper joints (never animated) and ~150 clip
   CRCs (mostly non-rifle classes and the fp bank). [D]
7. `skel+0x210` 16 B/bone and the `animCount×0x10` bank gap: unknown,
   never read by the sampling code found. [H]
8. `zaf_` MTLS/JSTS/SKNS: reference-model leftovers, skipped by the engine;
   JSET (two joint-index lists, e.g. 20 + 8 for human) purpose unknown. [H]

---

## 10. Ten-line summary for the lead
1. Bone identity everywhere = `crc32_lower(name)` (poly 0x04C11DB7): TNJA `+0`, JONT `+0`, model `skel` names all match (`DummyRoot 4446E9B8`, `bone_root 16E27226`, `bone_pelvis E6E47876`, `hp_weapons 2B960099`).
2. Rig = `zaf_` BIN_ `ZAFF{VERS,SIZE(matl,jset,skin,bones),MTLS,JSTS,SKNS,SKEL{JONT×n},PXYS,SHDS}`; JONT 0x2C = crc, rest quat xyzw, rest pos, parent/child/sibling CRCs. Human rig = 31 joints (table §2.1), BF1 `human_animation_0` ≡ BF2 `human_0`; model `skel` XFRM pos identical, rot = R(q)ᵀ.
3. TNJA 0x30 = `crc, rotOff[4], transOff[3], base[3], scale`; track i of a clip has translation iff MINA mask bit i.
4. TADA stream = `i16 v0` + ops `0x80 nn` hold / `0x81 i16` absolute / `i8` delta, one value per frame; rotation = v/2047 xyzw (absolute local quaternion), translation = base + scale·v (metres, local); constant `base` when untranslated.
5. Sampling: `f = trunc(t·(F−1))`, slerp shortest-path to f+1 (or F−1 clamp), lerp translation; loop clips have frame F−1 ≡ frame 0.
6. Timing: 30 fps; BF1 `t += dt·30/F`, BF2 `t += dt·30/(F−1)` (durations F/30 vs (F−1)/30); loop/reverse are runtime flags, not stored in the bank.
7. Root motion = `DummyRoot` track z (run 6.69 m/s, walk 3.87 m/s, dive 5.4 m/35 fr); strip it for in-place playback; `bone_root` carries height only.
8. Decoder verified: 39 422/39 422 streams end exactly on the next stream, BF1↔BF2 shared clip decodes bit-identically, FK of idle = standing figure; 4 zero-norm keys in shipped data need a guard.
9. Vectors: §7 (MINA @0x20113C, TNJA @0x204004/0x204064/0x204184, streams @0x214D07/0x214DB3/0x2151D7/0x21520D, frames 0/10/34, JONT @0x287CAC).
10. Start with `human_rifle_stand_idle_emote` (anim_0 #31, 71 fr, no translation) then `human_rifle_stand_runforward` (anim_2 #28, 19 fr) and `human_rifle_diveforward` (anim_2 #9, 35 fr); `python3 animkeys.py COMMON.LVL human_animation_2 human_rifle_diveforward --rig human_animation_0 --csv out.csv` dumps everything.
