# RE: collision LAYERS — which bodies a soldier / vehicle / ordnance collides with (2026-09-03)

Fable RE agent report, lead-verified the same day. **Lead reproductions** (each
re-run by the lead on the raw discs / decomp before this file was accepted):
fnv1a of the lowercased key names (`vehiclecollisiononly` = C3F25CBB,
`targetablecollision` = D23DBAF8, `soldiercollision` = 5DFDC07F,
`vehiclecollision` = DE5365A1, `ordnancecollision` = FB2BDF07,
`buildingcollision` = 9828F43F, `terraincollision` = AFE693CE); the sentinel the
agent left open, 0x5C6E1222, is fnv1a("clear") (BF2 POL1's shields author
`SoldierCollision = CLEAR`); BF1 `FUN_001676b0` ORs the new body's bit into the
five subsets +0x94/+0x98/+0x9c/+0xa0/+0xa4 (decomp lines 6237–6252); BF1
`FUN_001fb6e8` at 0xc3f25cbb adds to subset 4 and clears the bit from subsets 2
and 0x200 (lines ~109347–109365); BF2 `FUN_001606e8` initialises the MASK temp
once, sets it only on 0x4b53414d, applies it at DATA into body+0x44 and never
clears it (decomp lines 448/461/496–497); BF2 MYG1.LVL raw prim chunk of
`myg1_bldg_large_platform_b`: MASK 0x11 @0xD665C (`p_-so-…`), 0x13 @0xD67F4
(`p_-svo-…`), 0x02 @0x10667C (`p_-v-rob`); the entc PROP census over all 32 map
LVLs (scratch `entc_collision_keys.txt`): 93 classes carry a collision-layer
key — SoldierCollision 94 lines, OrdnanceCollision 123, VehicleCollision 41,
VehicleCollisionOnly 112, Building/Terrain 3 each (cloud car). What was NOT
independently re-read by the lead: the BF2 subset offsets and the collide-path
functions of §4 (accepted as the agent's [V] with the addresses it cites).

Port consequence (PLAN Log 2026-09-03, A.4): the session-7 name rule
(`*vehicle*` / `*flyer*` / `*ordnance*` prims do not block soldiers) is replaced
by the class subsets; HOLD 13's "layer masks" item is answered for the subset
SELECTION, and stays open for the collision RESPONSE (§7).

---

# RE: collision PRIMITIVE layer masks — which `prim` bodies a soldier / vehicle / ordnance uses (2026-09-03, Fable, analysis only)

Corpora: `analysis/SWBFRONT.ELF-decomp.c` (BF1), `analysis/SWBFRNT2.ELF-decomp.c` (BF2),
the `*-functions.txt` / `*-stringrefs.txt` beside them, `analysis/a4-bf{1,2}-edges.txt`,
the raw ELFs `extracted/bf{1,2}/SWBF*.ELF` (read for strings/vtables) and the disc LVLs
under `extracted/bf{1,2}/lvl/DATA/_LVL_PS2/` (scanned with throwaway Python in the
scratchpad: `scan_mask.py`, `scan_namemask.py`, `entc_props.py`). Nothing under
`engine/`, `analysis/`, PLAN or HANDOFF was touched.

Tags: [V] verified in the decomp / on the discs, [D] derived, [H] hypothesis.

The BF2 flyer override is a measured exception to the generic entity rules
below; see section 9 (2026-11-10) before applying them to flyer hulls.

## 0. Headline (the answer the port needs)

1. **Prim NAMES are never parsed.** Neither engine scans the name for `-so-` letters
   [V]. The name is only hashed (FNV-1a, case-insensitive) and stored so that ODF
   properties can refer to the body by name.
2. **The per-layer body selection lives in the entity CLASS (entc PROP list), not in the
   prim chunk.** `SoldierCollision`, `VehicleCollision`, `OrdnanceCollision`,
   `BuildingCollision`, `TerrainCollision`, `TargetableCollision` and
   `VehicleCollisionOnly` each name bodies (prim NAME or the literal `collisionmesh`);
   each property sets the model's subset for ONE layer [V].
3. **Default with no property: EVERY body — the coll mesh AND every prim, flagged or
   not — is in every one of the five subsets (soldier, vehicle, building, terrain,
   ordnance)** [V both engines]. An unflagged `p_cube` on a class with no collision
   properties DOES block soldiers.
4. **`nab2_bldg_b_19` is resolved by its class, not by a name rule** [V on both discs]:
   `VehicleCollision = p_cube`, `VehicleCollision = p_cube1`,
   `SoldierCollision = collisionmesh`, `OrdnanceCollision = collisionmesh`. So soldiers
   (and bolts) see only the mesh; vehicles see only the two boxes. That is why soldiers
   stand on the plaza inside the boxes.
5. **BF2 adds a per-prim `MASK` u8 chunk** (the lead's "no mask chunk" finding holds for
   BF1 only — 12 of 21 BF2 map LVLs carry MASK, 3,000 of 6,000 BF2 prims) [V]. The
   PC-tool letters are baked into it: **s=0x01 v=0x02 b=0x04 t=0x08 o=0x10** [V by
   correlation on ~6,000 prims, zero contradictions]. MASK seeds the body's subset
   membership at add time; the ODF properties then override per layer. MASK absent or
   0 = all five subsets. BF1 has no per-prim flag field at all.
6. The soldier-vs-building test uses the building's **soldier subset** and the
   soldier's own **building subset**; a ray (bolt, reticle, LOS) with the trailing flag
   uses the target model's **ordnance subset**; the terrain object uses the moving
   object's **terrain subset** [V].

## 1. The `prim` loader

### BF1 `FUN_001606c0` (SWBFRONT.ELF-decomp.c lines 430–597) [V]
Children handled: `INFO` 0x4f464e49 (128-byte model name + u32 count), `NAME`
0x454d414e (→ 128-byte buffer *(decompiler detail omitted)*), `PRNT` 0x544e5250 (→ *(decompiler detail omitted)*),
`XFRM` 0x4d524658 (12 floats read as 3 rows + translation, w = 0/0/0/1.0),
`DATA` 0x41544144 (u32 kind, f32 a b c). **No `MASK` (0x4b53414d) and no other tag
is looked for** — grep of 0x4b53414d over the BF1 decomp: zero hits [V].

On `DATA`:
*(decompiler excerpt omitted from the public copy)*
The format string at 0x526500 is `%s%d` (read from the ELF) [V]: prims are keyed
`<modelname><index>` (e.g. `nab2_bldg_b_190`, `nab2_bldg_b_191`), and the class loader
enumerates them with the same `%s%d` (0x530c48) until a lookup fails.

**What is stored per primitive** (`CollisionBodyPrimitive`, RTTI 0x5265f0; ctor
`FUN_00160378` → base `CollisionBody` ctor `FUN_00160040`) [V]:
`+0x00..0x3f` XFRM (4×float4), `+0x40` hash(NAME) (`FUN_00444b88`), **`+0x44` =
hash(PRNT)** (*(decompiler detail omitted)*), `+0x48` vtable, `+0x50..0x6f` the
`CollisionPrimitive` shape (`+0x54` radius/hx, `+0x58` height/hy, `+0x5c` hz, `+0x64`
kind; cf. `FUN_00160ab0`). **There is no flags/mask word in BF1's body.** Default
membership is decided when the body is added to a model (§2).

### BF2 `FUN_001606e8` (SWBFRNT2.ELF-decomp.c lines 403–583) [V]
Same tags plus **`MASK` 0x4b53414d**:
*(decompiler excerpt omitted from the public copy)*
`FUN_003cb350` reads exactly 1 byte (*(decompiler detail omitted)*). BF2's
`CollisionBody` layout moved the parent hash to `+0x48` and the vtable to `+0x4c`;
`+0x44` is the **mask byte**, default 0 (`FUN_00160380`, `FUN_00160328`).

**Engine quirk [V]:** *(decompiler detail omitted)* is initialised once (*(decompiler detail omitted)* at entry) and only
written by `MASK`; it is never cleared after a `DATA`. A prim WITHOUT a MASK that
follows a prim WITH one in the same chunk inherits the previous byte. The discs hit
this: 26 BF2 prim chunks mix masked and unmasked prims, and **209 unmasked prims
follow a masked one** (e.g. COR1 `cor1_bldg_minor_hall_06` `p_cube36`, `p_cube154`…
`p_cube161`) — on the real BF2 they carry the earlier prim's letters, not "all
layers". A faithful port must replicate the carry-over (or decide, and log it).

## 2. Bodies → CollisionModel → per-layer subsets

### BF1 `CollisionModel` (RTTI 0x526808) [V]
`+0x10..` body pointer array (max 0x20), `+0x90` count, subsets = one u32 bitmask
over body indices per layer:
`+0x94` (default / soldier 2), `+0x98` (4), `+0x9c` (8), `+0xa0` (0x40), `+0xa4`
(0x200), `+0xa8` (0x400), `+0xac` (0x1000). Getter `FUN_00168350(model,&layer)`
returns the address of the subset for a layer id; setter `FUN_00168400(model,&layer,&bits)`.

**Add-body `FUN_001676b0` (lines 456–472 of the module) — the default rule [V]:**
*(decompiler excerpt omitted from the public copy)*
`+0xa8` (0x400) and `+0xac` (0x1000 = Targetable) stay empty unless a property fills
them. Model init `FUN_00167610` zeroes all seven.

**Which bodies a class gets [V]:** the common class parser `FUN_001e3678` handles
`GeometryName` (0x47c86b4a): loop `sprintf("%s%d", name, i)` → prim table 0x564b88 →
`FUN_001676b0(class+0x13e4, prim)` for every prim of the model (line 92641 ff.).
The building/prop base parser `FUN_001c0b10` then looks the geometry name up in the
coll-mesh table 0x566bc8 and adds ONE `CollisionBodyMesh` body named
`"CollisionMesh"` (string 0x52e318; `FUN_00160da0(new(0x60), mesh, "CollisionMesh",
identity 0x66ef20, 0)`) to the same model. So the mesh and the prims are peers in one
body list; nothing about the mesh's presence changes how prims are added (§3).

**ODF property → layer selector (BF1, dispatch in `FUN_001e3678`) [V]:**

| property (FNV-1a, lowercase) | hash | handler | layer id | subset |
|---|---|---|---|---|
| `SoldierCollision` | 0x5dfdc07f | `FUN_001e5200` | 2 | +0x94 |
| `VehicleCollision` | 0xde5365a1 | `FUN_001e4fa8` | 4 | +0x98 |
| `BuildingCollision` | 0x9828f43f | `FUN_001e4ee0` | 8 | +0x9c |
| `TerrainCollision` | 0xafe693ce | `FUN_001e5070` | 0x40 | +0xa0 |
| `OrdnanceCollision` | 0xfb2bdf07 | `FUN_001e5138` | 0x200 | +0xa4 |
| `TargetableCollision` | 0xd23dbaf8 | `FUN_001e52c8` | 0x1000 | +0xac |
| `VehicleCollisionOnly` | 0xc3f25cbb | inline in `FUN_001fb6e8` (line 109347) | 4, and removes from 2 and 0x200 | |

Handler shape (all six identical, e.g. `FUN_001e5200`):
*(decompiler excerpt omitted from the public copy)*
Repeated lines accumulate (per-class static `DAT_005177xx`, reset by the
`GeometryName` handler). `"none"` (0xada7afdb) empties the subset; the second
sentinel 0x5c6e1222 was not resolved (not none/off/false/0/no/all/null). The body
name `collisionmesh` resolves to the mesh body because `FUN_00160da0` names it
`"CollisionMesh"` and the hash is case-folded.

`VehicleCollisionOnly` (BF1 line 109347, `FUN_001fb6e8`) [V]:
*(decompiler excerpt omitted from the public copy)*
This is exactly `kas2_bldg_hut_round`: its entc carries `0xc3f25cbb = p_vehicle_3`
[V, KAS2.LVL], so the 6×20 m cylinder is vehicle-only.

### BF2 `CollisionModel` [V]
Body array at `+0x04`, count `+0x104`, subsets are u64 pairs: `+0x108` (default /
soldier 2), `+0x110` (4 and 8 — vehicle and building share one subset in BF2's
setter), `+0x118` (0x10), `+0x120` (0x80), `+0x128` (0x400), `+0x130` (0x800),
`+0x138` (0x2000), `+0x140` (0x40, getter only). Getter `FUN_00168af0`, setter
`FUN_00168bb0`, body-by-name `FUN_001681c8`.

**Add-body `FUN_00167048` (SWBFRNT2 line ≈5966) — the MASK rule [V]:**
*(decompiler excerpt omitted from the public copy)*
BF2 property handlers (dispatch in `FUN_00214600`, same hashes as BF1) [V]:
`SoldierCollision` → `FUN_00216378` selector 2 (+0x108); `VehicleCollision` →
`FUN_002160a8` selector 8 (+0x110); `BuildingCollision` → `FUN_00215fb8` selector
0x10 (+0x118); `TerrainCollision` → `FUN_00216198` selector 0x80 (+0x120);
`OrdnanceCollision` → `FUN_00216288` selector 0x400 (+0x128); `TargetableCollision`
→ `FUN_00216468` selector 0x2000 (+0x138). So BF2's layer ids are soldier 2,
vehicle 4/8, building 0x10, terrain 0x80, ordnance 0x400, targetable 0x2000 — which
is why BF2's ordnance ray mask is 0x90 (= 0x80 terrain | 0x10 building) where BF1's
is 0x48 (= 0x40 | 8) (re-blaster-damage §10.7 masks explained) [D].

**Letter → MASK byte, measured on the BF2 discs (`scan_namemask.py`, all 21 BF2 LVLs)
[V]:** `-s-` → 0x01 (21), `-v-` → 0x02 (176), `-o-` → 0x10 (58), `-so-` → 0x11
(2,296), `-vo-` → 0x12 (79), `-sov-`/`-svo-`/`-vso-` → 0x13 (272), `-bt-` → 0x0c
(1: `cis_walk_droideka p_-bt-building`), `-vbt_` → 0x0e (1), `-sv_` → 0x03 (9), no
letters → no MASK (3,002). Hence **s=1, v=2, b=4, t=8, o=0x10**, and by
`FUN_00167048` those are precisely soldier / vehicle / building / terrain / ordnance
subsets. The PC convention is confirmed, not assumed. Kind-4 boxes, kind-2 cylinders
and kind-0 spheres all carry it the same way.

## 3. Does a coll mesh change how prims are used?

**No, not by itself [V].** The mesh is one more body (`CollisionBodyMesh`, named
`CollisionMesh`) in the same list, added by `FUN_001c0b10` AFTER the prims, through
the same `FUN_001676b0` default (all five subsets). Nothing in the add path, the
subset getters or the collide path tests "has mesh". What separates them in practice
is authored per class: e.g. `nab2_bldg_b_19` names `collisionmesh` for soldiers and
ordnance and `p_cube`/`p_cube1` for vehicles. `kam_bldg_mushplatform1` (mesh
`SHELL-Bottom` + `p_collision_floor`, manifest `collision:` entry has
`glb: Some(...)`) has NO collision properties (KAM1.LVL entc: only GeometryName and
FoleyFXGroup) → soldiers collide with the mesh AND the floor cylinder [V].

`FUN_00168a60` does consult two model fields (`model+8`, `model+0xc`, zeroed in
`FUN_00167528`) to promote a pair to layer 4 or 8 — these are not the mesh (the mesh
is an ordinary body) — their meaning is left open [H: dynamic/attached-model flags].

## 4. The soldier's collision query — which subsets are used [V]

Manager (`CollisionManager`, RTTI 0x5266f8, singleton `DAT_0051568c`) keeps one
object list per layer id: `+0x18` (1), `+0x2c` (2), `+0x40` (4), `+0x54` (8),
`+0x68` (0x10), `+0x7c` (0x100), `+0x90` (0x800); the terrain object at `+0xbc`
(mask bit 0x40) and a second special object at `+0xc0` (bit 0x80). Registration
`FUN_00164198` puts an object in the list of its own layer id; the broadphase
`FUN_001627d0(mgr, min, max, cb, mask, ctx)` walks the lists whose bit is set.

Per-tick pass `FUN_00163008` [V]:
*(decompiler excerpt omitted from the public copy)*
`FUN_00163658` moves each object of the list through its sub-steps: first the terrain
object's vtbl+0x18 (`(*(terrain+0xc0)+0x18)(terrain, obj, out)` = **`FUN_00212530`**,
the `EntityTerrain` vtable at 0x5330f0), then every candidate gathered by
`FUN_001627d0(mask 0xd)` via the moving object's own vtbl+0x18.

The base collide, `CollisionObject` vtbl+0x18 = **`FUN_00168a60(self, other, out)`**
(vtable 0x526860; the building vtables 0x52e3d8/0x52e788/0x52eb20/0x52eec8 and the
entity vtable 0x52dda8 all keep it) [V]:
*(decompiler excerpt omitted from the public copy)*
So **a soldier (layer 2) against a building (layer 8) tests the building's
`SoldierCollision` subset (+0x94) against the soldier's `BuildingCollision` subset**,
mesh polygons and prims alike; the narrowphase is per body pair (mesh body vs the
soldier's box/cylinder, prim vs prim). The terrain object (`FUN_00212530`) uses the
moving object's `TerrainCollision` subset (`FUN_00168350(model, &0x40)`). The
response functions (vtbl+0x1c, `FUN_00212668`/`FUN_00213aa0`-area for soldiers) were
NOT traced — the step/slope/push-out rule stays open (HOLD 13 still holds for that
part).

Layer-id assignment per entity type is [D], not read off a constructor: it follows
from the property selectors (SoldierCollision = 2, VehicleCollision = 4,
BuildingCollision = 8), from `FUN_00163008`'s pass structure (list 2 is moved against
1|4|8 + terrain; list 4 against buildings), and from the ray masks already in the
log (aim-assist 0x5e = 2|4|8|0x10|0x40; ordnance 0x48 = 8|0x40). The direct
`obj+0x24 = <layer>` stores were not found by grep (the layer often comes from a
class table, *(decompiler detail omitted)*); layers 1, 0x10, 0x100, 0x800 remain
unnamed [H: 0x10 walker/droid-like — aim-assist targets it].

Rays [V]: `FUN_00162c60` → per object vtbl+0x20 = `FUN_001688e8` →
*(decompiler detail omitted)*: with *(decompiler detail omitted)* only bodies in the **ordnance
subset (+0xa4)** are tested, else all bodies. Ordnance (`FUN_002764c0`, line 197643:
`FUN_00161d68(len, ..., 0x48, 1)`), the soldier reticle (`FUN_00204748`, mask 0x4e,
flag 1) and LOS (`FUN_00162e88`, *(decompiler detail omitted)*) pass the flag; aim-assist
acquisition (`FUN_00223850`: `..., 0, 0` → default 0x5e, flag 0) tests all bodies.

BF2's collide is `FUN_00169328` (same shape: two layer ids, promotion to 8,
`FUN_00168af0` twice, `+0x18`-model pair table) [V by structure; not line-read].

## 5. Vehicle / ordnance masks (for the port's table) [V]/[D]

| | BF1 layer id → subset | BF2 layer id → subset | BF2 MASK letter |
|---|---|---|---|
| soldier | 2 → +0x94 (default slot) | 2 → +0x108 | s = 0x01 |
| vehicle | 4 → +0x98 | 4 or 8 → +0x110 | v = 0x02 |
| building | 8 → +0x9c | 0x10 → +0x118 | b = 0x04 |
| terrain | 0x40 → +0xa0 | 0x80 → +0x120 | t = 0x08 |
| ordnance | 0x200 → +0xa4 | 0x400 → +0x128 | o = 0x10 |
| targetable | 0x1000 → +0xac (default empty) | 0x2000 → +0x138 | — |
| (unused) | 0x400 → +0xa8 | 0x40 → +0x140 / 0x800 → +0x130 | 0x20 |

Query masks seen: BF1 ordnance ray 0x48 (building|terrain), reticle 0x4e, aim 0x5e;
BF2 ordnance 0x90 (building|terrain). Soldier world pass: mask 0xd + terrain.

## 6. What the port should do (derived)

- Read the class's PROP list for the seven hashes above; build per-layer body sets:
  start with ALL bodies (mesh + prims) in soldier/vehicle/building/terrain/ordnance,
  then for each property line replace/accumulate as `FUN_001e5200` does (`none` →
  empty; repeated lines OR together; `VehicleCollisionOnly` adds to vehicle and
  removes from soldier+ordnance). Property order in the entc is authoritative
  (GeometryName first, verified on the dumps).
- BF2 bake: read `MASK` u8 per prim; seed sets from its bits (0 → all five); replicate
  the stale-MASK carry-over inside a chunk (209 prims depend on it) — or log a
  deliberate deviation.
- Drop the `*vehicle*`/`*flyer*`/`*ordnance*` NAME rule (PROVISIONAL in HOLD 13): it
  is not what the engine does; `p_vehicle_3` is excluded by `VehicleCollisionOnly`,
  `p_cube` by `SoldierCollision = collisionmesh`.
- Soldier walk/ground: use the SOLDIER set of each model — mesh polygons and prims
  together; do not special-case "model has a mesh".

## 7. Negatives / not closed

- No name parser exists in either ELF (searched the loaders, the class parsers, and the
  `CollisionBody` ctors; the only NAME use is `FUN_00444b88` hashing) [V].
- BF1 `CollisionBody +0x44` is the PRNT hash, not a mask [V].
- Sentinel value hash 0x5c6e1222 in the six handlers: unresolved.
- `model+8` / `model+0xc` promotion flags in `FUN_00168a60`: unexplained.
- Entity → layer-id constructor stores: not located (class-table indirection);
  the mapping in §5 is derived, with three independent consistency checks.
- Soldier collision RESPONSE (vtbl+0x1c), step height, slope, "ground under the
  soldier" selection among hit bodies: not traced (out of the 90-minute bound). Next
  step: read `FUN_00212668`/`FUN_00213aa0`/`FUN_00212bb0` (BF1 soldier module,
  0x211290–0x214ff8) and the narrowphase `(*(model+0xb0)+0xc)` table at 0x5267f0;
  a PCSX2 trace of `FUN_00168a60`'s subset arguments on NAB2 would confirm §4 live.

## 8. Coordinator's mid-task questions, answered point by point

- **Where the ELFs consume the hashes** [V]: BF1 `FUN_001e3678` (common
  `EntityGeometry` class parser, lines 92742/92786/92817/92830 → `FUN_001e5200`
  SoldierCollision, `FUN_001e4ee0` BuildingCollision, `FUN_001e52c8`
  TargetableCollision, `FUN_001e5070` TerrainCollision; 93117 → `FUN_001e4fa8`
  VehicleCollision; 93129 → `FUN_001e5138` OrdnanceCollision); the same six
  comparisons also appear in the vehicle/soldier class parsers at lines 89958,
  101998, 109269 (`FUN_001df3f0`, `FUN_001f0648`, `FUN_001fb6e8`). BF2:
  `FUN_00214600` (lines 127847–128286 → `FUN_00216378`, `FUN_00215fb8`,
  `FUN_00216468`, `FUN_00216198`, `FUN_002160a8`, `FUN_00216288`) and the class
  parsers at 80727, 125368, 137532, 146057.
- **Built 2026-11-04:** the vehicle subset is `Layers::VEHICLE`, from the same rule as the other two, and flyer contact and
  pilot rays query it (`re-space-assault.md` §10). Hover and walker traffic still use their named soldier surrogate.
- **Built 2026-11-05:** a flyer's own contact queries `Layers::FLYER`. Entity prop `0xF31D1478` (byte +3 of the model,
  B2:145727-145728, B1:109208) picks which subset that is: set means the vehicle subset, clear means the soldier subset
  (`FUN_00169328`, B2:7790-7870). The bake carries it as `flyer_uses_vehicle`. `re-space-assault.md` §11.
- **0xC3F25CBB = `VehicleCollisionOnly`** (FNV-1a of the lowercased key; found by
  brute force over `<prefix>Collision<suffix>` and confirmed by the handler's
  semantics). Consumed in BF1 `FUN_001fb6e8` line 109347 and BF2 `FUN_0022f6e8`
  line 146264 (both the hover/vehicle-side class parsers) [V]. Its effect is quoted in
  §2: add the named body to the vehicle subset, clear it from the soldier and
  ordnance subsets.
- **What is done with the named node**: the value is hashed and matched against every
  body's `+0x40` name hash (`FUN_00167a48` BF1 / `FUN_001681c8` BF2); the body's INDEX
  bit is OR-ed into a per-class accumulator and the accumulator is written as that
  ONE layer's subset (`FUN_00168400` / `FUN_00168bb0`). It is a per-model subset per
  layer, not a per-node mask word [V].
- **Default for a node named by no key** [V]: it is in all five subsets (soldier,
  vehicle, building, terrain, ordnance) from `FUN_001676b0` (BF1) or
  `FUN_00167048` with MASK == 0 (BF2); never in targetable.
- **Does naming a node under one key remove it from the other layers?** No [V]. A
  key REPLACES its own layer's subset with the accumulated named set (so the bodies
  NOT named drop out of that layer), and leaves every other layer untouched. Only
  `VehicleCollisionOnly` explicitly removes from soldier + ordnance. Example
  `nab2_bldg_b_19`: soldier = {mesh}, ordnance = {mesh}, vehicle = {p_cube, p_cube1},
  building = terrain = {mesh, p_cube, p_cube1} (untouched defaults).
- **`FlyerCollision` (0x090afb1f)**: not consumed by either ELF (zero hits for the
  immediate in both decomps) [V negative]. Neither is the PC-only
  `BuildingCollisionPrim` (0x8cf29543) — it IS present in BF1 `FUN_001e3678` line
  ≈92720 as `0xffffffff8cf29543`, sharing the `0x74e63ee8` branch (both jump to
  *(decompiler detail omitted)* with *(decompiler detail omitted)*); not read further.
- **Does the BF2 ELF parse letters from the NAME?** No [V]. `FUN_001606e8` copies the
  NAME into a 128-byte buffer and the ctor hashes it; the letters are consumed by the
  PC exporter/munger, which writes them as the 1-byte `MASK` chunk (§1, §2 table).
  The BF2 engine reads only that byte.


## 9. BF2 flyer exception: retain the fuselage mesh (2026-11-10) [V][M][O]

The generic entity rule above is not the BF2 flyer's SoldierCollision and
OrdnanceCollision rule. `FUN_0020fd58` handles these keys itself. In the current
local decompilation, its GeometryName branch (lines 124396-124419) loads the
mesh and seeds both flyer accumulators (the globals at 0x5fcf44 and 0x5fcf48) with
that body's bit. Its SoldierCollision branch (124684-124698) and
OrdnanceCollision branch (125151-125162) add named bodies to those seeded
accumulators. Neither branch handles `none` or `clear` as a special value:
an unresolved body name leaves the existing set alone. The generic entity
handlers remain separate, with initially empty accumulators.

The live Space Hoth X-wing class is `0x19b22c0`. Its model at class+0x168 has
13 bodies. Body 12 is `CollisionMesh`, name hash `0x29173102`. The soldier and
ordnance masks both read `0x10ff`: the fuselage mesh plus the eight wing/laser
primitives. Vehicle, building and terrain read `0xe00`, the three spheres;
targetable reads `0x1000`, the mesh alone. OpenBF previously used the generic
rule for this flyer and therefore retained only the eight named wing/laser
primitives in its ordnance subset. A missile aimed at the model origin could
pass through the absent fuselage.

Two native locked TIE torpedo launches corroborate the mask measurement. One
uses a player craft translated and rotated to trail the native X-wing by
approximately 350 m. The other restores that state, changes both craft
matrices, and repeatedly resets the target matrix. The target's native flight
state reconstructs its orientation during updates, so the second shot is not
a frozen-orientation or natural-flight hit-rate experiment. Both use retail
L1 firing, missile guidance and target assignment; health and weapon values
are not written. Both register 1600 damage including the intervening target
regeneration. In the first, the impact endpoint in the X-wing's frame is
approximately `(0.020109, 0.458971, -3.988204)`, on the fuselage mesh's rear
face at z=-3.988209.

The port replays each recorded chord against the target's same-boundary
matrix. The native bullet's previous/current positions are instance+0x100
and +0x50 (`FUN_00313600`, current lines 292053-292067), not consecutive
previous-position samples. A 1 cm extension of the test ray covers recorded
matrix/endpoint rounding; it does not change simulation collision radii.
Both native contacts failed against the old hull and pass against the corrected
one. The correction applies only to BF2 flyer hull setup; generic entities,
BF1 flyers, targetable points and the flight kernel are unchanged. The existing
baked mesh and primitive data are sufficient; no asset rebake is needed.

Local raw captures and masks are in
`engine/artifacts/lead_tmp/republic-parity/native-{near,aligned}-xwing.json`
and `native-xwing-masks.json`, Tracking run `01M72PBHAM092X7D4YBF8P58J5`.
`re-ordnance-kinds.md` section 4.8 records the complete secondary loadout sweep.
