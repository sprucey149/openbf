# OPUS RE (loop lead, 2026-09-17, lane 2) - [O] UNAUDITED: no Fable pass

**Queue SW — "the wall that pins MUS1's team 0 and POL1's team 1": which PS2 rule the
port's collision breaks.** The answer is candidate **(a), a BF2 door** — but not "a door
the mission Lua opens": BF2 has an engine entity class `door` that opens **by itself** when
a soldier comes within the ODF's `AnimationTrigger` radius of a named hardpoint. The mission
Lua never opens one — it only LOCKS one; where a script does work a doorway (POL1's airlock)
the object is a plain `prop` whose named animation the script plays. The port has no door
entity: `bake` writes the door panel's collision box at its bind (closed) pose into the static
world and nothing ever moves it, so **50 doors on 5 BF2 maps stand as permanent walls across
authored planning arcs**.

Tags: [V] read it (lines cited), [D] derived, [H] hypothesis; `[O]` = this file's evidence
is one Opus pass plus one Opus verifier, with no Fable audit. Scratch, every script and
every number: `engine/artifacts/lead_tmp/re/stuck-wall/`.

## 0. The answer in one table

| candidate (the queue entry's own order) | verdict |
|---|---|
| (a) a BF2 door the mission Lua opens | **YES for the door, NO for the Lua** — the engine opens it; the Lua locks it, and the doors a script DOES animate are plain `prop`s [V][O] |
| (b) a collision primitive the disc does not apply to soldiers | **NO** — the door's `p_door` box is in the disc's soldier subset too; it is the BOX THAT MOVES [V][O] |
| (c) an arc the disc's own path data does not offer | **NO** — every stuck leg is an arc of the disc's own `plan` chunk, and MUS1's conquest Lua explicitly UNBLOCKS the one the 7-unit knot stands on [V][O] |

## 1. The port's symptom, re-measured (`stuck-census/land/main_census_1.err`, lead-run on main `f6cbf90`)

**All 13 `wall` verdicts on the two maps are doors**, and a 14th unit is queued behind one:

| map | stuck units | position | leg (arc) | nearest door instance | dist. to the panel | to `hp_door` | trigger |
|---|---|---|---|---|---|---|---|
| mus1 | 3,4,5,8,10,16 (`wall`) + 14 (`body`) | (−47.3,68.0,90.8) | hub 3→31, locus **arc 18 `Connection74`** | `hangardoor` (`mus1_prop_door_tri`) | 1.2–1.9 m | 2.10–2.72 m | 4.0 m |
| mus1 | 7 (`wall`) | (−14.4,68.0,90.1) | hub 2→19, locus arc 17 | `door_cont_11` (same class) | 1.6 m | 1.14 m | 4.0 m |
| mus1 | 23 (`wall`) | (5.8,66.0,17.6) | hub 58→57, arc 66 | `door_cont_9` (`mus1_prop_door_garage`) | 1.3 m | 1.25 m | 5.0 m |
| pol1 | 18,19,20,22 (`wall`) | (−39.1,25.0,81.7) | hub 0→3, **arc 0 `Connection59`** | `pol1_prop_door2` (`pol1_prop_door`) | 0.52 m | 1.23–1.97 m | 3.0 m |
| pol1 | 21 (`wall`) | (−39.7,25.0,49.8) | hub 1→2, **arc 1 `Connection60`** | `pol1_prop_door` | 0.59 m | 1.25 m | 3.0 m |

The distances are to the door PANEL, not to the nearest body: the mus1 knot is jammed into the
corner beside its doorway, so the nearest surface to its centre is the hangar's own wall at 0.01 m
(`nearest_surface.py`). What the door does is close the ROUTE, and that is what the flood-fill
below measures.

The remaining four stuck units on these maps are a different defect and stay open: mus1 unit 11
(`off-graph`, `floor NO_GROUND`, 8 m from the nearest door) and pol1 units 1 (`off-graph
[under-terrain]`), 26 and 27 (`no-arc`/`body` at (−91,21.7,−127)).

**Every stuck unit stands INSIDE the disc's own trigger sphere** (`trigger_reach.py`: 1.14–2.72 m
against a 3–5 m radius): on the PS2 the door in front of them would be open. [V][O]

**The door is what stops them, measured in the port's own world** (`door_passable.py`: grid
0.3 m at chest height, a cell free when the nearest collision surface is ≥ `SOLDIER_RADIUS`
0.5 m, flood-fill inside a 5 m-wide corridor around the arc, exact point-to-triangle distance
for `coll` meshes and exact box distance for prims):

| doorway | with every baked body | with the `base: "door"` instances' bodies removed |
|---|---|---|
| mus1 `hangardoor` (z 93.5 → 85.5) | **not reachable** | reachable |
| mus1 `door_cont_11` (z 93.5 → 85.5) | **not reachable** | reachable |
| mus1 `door_cont_9` (x 7.5 → 1.5) | **not reachable** | reachable |
| pol1 `pol1_prop_door2` (x −41.5 → −35.5) | **not reachable** | reachable |
| pol1 `pol1_prop_door` (x −41.5 → −35.5) | **not reachable** | reachable |

## 2. The disc's `door` class — the hops

Every citation names its enclosing function (§5 self-check 1). Decomp = `analysis/SWBFRNT2.ELF-decomp.c`.

1. **[V][O] The engine has a class label `"door"`.** `FUN_00173d10` — the entity class-label
   factory — allocates a **0x380-byte** class object and registers it under the string at
   **VA 0x0060fb68** with the constructor `FUN_001ec428` (lines 16384–16386:
   *(decompiler detail omitted)*).
   The bytes at that VA (file offset 0x4b0b68, BF2 `off = va − 0x160000 + 0x1000`) are the ASCII
   `door` — read from `extracted/bf2/SWBFRNT2.ELF`, not from a decompiled literal (§5 self-check 4).
   It sits in the same table as `building`, `animatedprop`, `soldier`, `flyer` …
2. **[V][O] `FUN_001ec5d0` is that class's ODF property handler** (lines 99895–99967; it is
   slot +0x20 of the class vtable 0x0061afc8 that `FUN_001ec428` installs, and its address occurs
   exactly ONCE in the whole ELF, at 0x0061afe8). It dispatches on exactly six keys, all by fnv1a
   hash: `OpenSound` 0x323f7450 → class+0x360, `CloseSound` 0x4fcf4562 → +0x368, `LockedSound`
   0x027b5cb2 → +0x370, `AnimationName` 0x98557a5e → `FUN_001ec588`, `Animation` 0xe145ee5d →
   the lookup result from the animation table at class+0x344, stored at class+0x340, and
   **`AnimationTrigger` 0x1d56b73b**, which is parsed with the format string at VA 0x0061afc0
   (`"%s %f"` — read from the ELF) into **up to THREE** (name-hash, float) pairs at
   class+0x348 / class+0x34c (*(decompiler detail omitted)* — and the class ctor `FUN_001ec428`
   clears exactly 0x18 bytes there, 3 × 8). Anything else chains to the base handler
   `FUN_0022f6e8`. The six names are the fnv1a preimages of those hashes.
3. **[V][O] `FUN_001eb3b8` is `EntityDoor`'s constructor** (header line 99176): it writes the
   vtable **0x0061b0e8** into `*this` (line 99199); its secondary vtable is 0x0061aff8, whose
   first word is 0xfffffe40 = −0x1c0. The class NAME comes from the module's static init
   `FUN_001ec950` (lines 100079–100080), which registers the strings at 0x0061b150 /
   0x0061b160 — `EntityDoor` / `EntityDoorClass` — from the same translation unit as
   `FUN_001eb3b8`, `FUN_001ec428` and `FUN_001ebb80`. (Adjacency alone would not do: the
   vtable's two header words and a zero word at 0x0061b14c sit between it and the string.)
4. **[V][O] `FUN_001ebb80` is the door's per-frame tick** (lines 99460–99716), reached through
   the thunk `FUN_001ec850` (lines 100011–100018: *(decompiler detail omitted)*) which is
   slot +0x0c of that 0x0061aff8 vtable — and that is its ONLY entry point: `FUN_001ebb80`'s
   address appears nowhere in the ELF as a pointer, the thunk's exactly once, at 0x0061b004.
   *(decompiler detail omitted)* is dt (*(decompiler detail omitted)* drives the frame advance, line 99640).
5. **[V][O] What the tick does.** For each of the ≤3 trigger pairs (lines 99556–99632): resolve
   the hardpoint by its name hash (`FUN_0057dc60(model, class+0x348+8i, out)`), transform it to
   world through the entity matrix (the VU macro block), then
   **`FUN_002b0260(radius = class+0x34c+8i, worldPos, outList, 0x14, 0, 7)`** — a RADIUS query
   over two entity lists (`FUN_002aff70` + `FUN_002b00e8`, the latter keeping an object iff
   `d² < radius²`, line 234413) whose last argument is a team bit-mask tested as
   `(mask >> (team+1)) & 1`, so 7 = any team — returning at most 0x14 objects (with a
   `DAT_00600548`/`DAT_00600560`-gated alternative, `FUN_003f0990(radius, pos, list, 0x14, 0, 2)`,
   lines 99610–99611, that this reading does not cover) — and if any hit's LAYER ID is **2**, set
   the "occupied" flag (lines 99614–99624). The id is read exactly the way
   `analysis/re-collision-layers.md` §4 describes the class-table indirection:
   `*(*(obj+0x10) + *(obj+0x14)*4 + 4)`, falling back to `*(obj+0x34)` when the table pointer is
   null; layer 2 is that RE's soldier layer ([V] there), which makes the "a SOLDIER opens it"
   reading [D][O] rather than [V].
   Then (lines 99640–99713): occupied and the LOCK bit (bit 0x22 of the u64 at `this+0x5f2`)
   clear → the animation parameter `this[0x4e9]` advances toward **1.0** at `dt·30/(frames−1)`;
   not occupied → it decays to 0 (lines 99651–99652). **The two sounds are EDGE-triggered by a
   latch, not played per frame:** `OpenSound` (class+0x360) plays only when latch bit 0x21 is
   clear and then sets it (99682–99687), `CloseSound` (+0x368) only when it is set and then
   clears it (99659–99661). The third arm — occupied AND the lock bit set — plays
   `LockedSound` (+0x370) when the time-since-empty accumulator `this[0x5f4]` exceeds 3.0 s
   (99691–99692); a locked door does not move at all.
6. **[V][O] The door also drives the room PORTAL it stands in**: *(decompiler detail omitted)* is the nearest
   object within 10.0 m (`0x41280000`) of the door from the singleton `FUN_005beba8` — the
   manager the `wrld` loader hands every `port` chunk to (line 18321, tag 0x74726f70 = `port`) —
   and every tick the door calls its vtbl+8 with "the animation parameter is > 0" (line 99696).
   Visibility, not collision; recorded so a later port does not re-derive it.

## 3. The disc's door DATA — read off MUS1.LVL / POL1.LVL

7. **[V][O] The class.** `entc` @0x97bc4 in MUS1.LVL: TYPE `mus1_prop_door_tri`, **BASE `door`**,
   9 PROPs — `GeometryName`/`AnimationName` = `mus1_prop_door_tri`, **`Animation` = `open`**,
   **`AnimationTrigger` = `hp_door 4`**, `OrdnanceCollision` = `p_door`, `OpenSound` = `door_open`,
   `CloseSound` = `door_close`, `LockedSound` = `vehicle_equip`, and 0xa1c9cbc5 = `metal_foley`
   (that last key's name is unreversed — [H]). POL1's `pol1_prop_door` is the same with
   `AnimationTrigger` = `hp_door 3.0`; `mus1_prop_door_garage` uses `hp_door 5`.
   **No `SoldierCollision` line**, so by `re-collision-layers.md` §0.3 the soldier subset is
   EVERY body: a shut door blocks soldiers on the disc too. That is why (b) is not the answer —
   the box is meant to MOVE, not to be ignored.
8. **[V][O] The rig.** `skel` @0x976cc: three nodes, `boneroot`, `bone_door` (child of boneroot)
   and **`hp_door`** (child of boneroot, bind position (0,0,−1.1098)) — the very hardpoint the
   ODF's `AnimationTrigger` names. `prim` @0x97b28: NAME `p_door`, **PRNT `bone_door`** — the
   door's ONE collision body hangs off the bone the clip animates. POL1's door has six boxes on
   `bone_door1`/`bone_door2`.
9. **[V][O] The clip moves it.** The `zaa_` bank named `mus1_prop_door_tri` holds exactly ONE
   clip, MINA name CRC **0xd02e3793 = crc32_lower("open")**, **31 frames** (BF2: (31−1)/30 =
   **1.00 s**), 3 tracks; track 2's bone CRC 0x3b9f76eb = crc32_lower(`bone_door`) and the MINA
   bone mask 0x4 marks it as the one with translation streams. Decoded (`anim_door.py`, the TADA
   rules of `analysis/re-anim-keys.md`): `bone_door` goes from (0.0000, −0.0000, 0.0001) to
   **(0.000, 3.707, 1.409) — 3.965 m of travel in 1.00 s**. POL1's two leaves slide **±2.250 m**
   along local X in the same time. The panel — and its collision box — leaves the doorway.
   `mus1_prop_door_garage`, whose box hangs off `boneroot` rather than a door bone, is the same
   rule with a different bone: its clip (36 frames = 1.167 s) translates **`boneroot` itself** by
   (1.218, 3.221, 0) = **3.444 m**, lifting the whole model. Every door clip also carries two
   rotation-only tracks for `dummyroot` (crc 0x4446e9b8), an editor node that never moves.

## 4. The Lua layer (PC modtools source, so [D][O] for the PS2 disc)

`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/SourceCode/Common/scripts/` is the
retail PC mission-script source, not the PS2 MISSION.LVL, so these are [D] until someone reads
the disc's compiled Lua:

- **A `door`-class door is never OPENED from script — it is only LOCKED.** `MUS/mus1c_c.lua`
  (the campaign) has `SetProperty("hangardoor", "IsLocked", "1")` and the same for
  `door_cont_1..9`; the four conquest scripts of these two maps contain **no `SetProperty` of any
  kind**, live or commented, so nothing locks the five doors that pin our units.
- **A scripted doorway is a different animal, and POL1 has one:** `pol1{c,g}_con.lua` bind
  `PlayAnimation("Airlockopen")` / `("Airlockclose")` to a command post's capture
  (`OnObjectRespawnName`/`OnObjectKillName` on `LockCon01`, lines 53–54). That object is
  `Pol_prop_airlock_door`, whose entc BASE is **`prop`**, not `door` — a plain prop whose named
  animation the mission drives — and its two instances sit 185–225 m from the stuck loci. So BF2
  has TWO doorway mechanisms and only the first one is in this file's scope. [V][O]
- `BlockPlanningGraphArcs` / `UnblockPlanningGraphArcs` (180 / 178 occurrences tree-wide,
  commented-out lines included) are the Lua face of the **`plnp`** route patches `mungefs::plan`
  already parses — and **`MUS/mus1c_con.lua:18` calls `UnblockPlanningGraphArcs("Connection74")`
  at mission start**: the very arc the 7-unit knot is stuck on, the one the `hangardoor` stands
  across (plnp group 2, the only arc in it). The disc's conquest AI is explicitly told to walk
  through that door. POL1's conquest scripts have only commented-out arc calls.
- So the port ignoring `plnp` (it always uses record 0, all groups open) is NOT what stuck these
  units — on this map the disc's own script asks for exactly the routing the port already does.
  (DR3, 2026-09-25: the port now applies `plnp` — `core::route_patch`, mask 0 at load as
  `FUN_002ead18` zeroes +0x2c — and the missions' start-of-round Block/Unblock calls ride
  `OPENBF_PLNP_MISSION=1`; MUS1's are all unblocks, so its routing is unchanged either way.)

## 5. What the port does today

- `game::assets::Scene::collision_world` (`engine/crates/game/src/assets.rs:468-489`) composes a
  prim's XFRM straight onto the instance matrix — "PROVISIONAL: its `PRNT` node's own transform
  is assumed identity". There is no animation, no door entity and no moving body: the world is
  built once and its broadphase with it.
  **Corrected 2026-09-23 (cleanse):** now :472-492 (the
  PRNT-identity comment at :490-491) — L's landing shifted it.
- Nothing in `engine/crates/` implements a door. The two places that know they are missing say so:
  `core/src/nav.rs:175` and `game/src/layout.rs:182` ("toggles doors and shields at runtime, which
  nothing drives").
- Scale, port-wide (`door_census_all.py`): **66 `base: "door"` instances over 5 baked maps —
  bf2_mus1 25, bf2_tan1 16, bf2_kam1 12, bf2_pol1 8, bf2_dea1 5 — and 50 of them stand within
  1.5 m of an authored plan arc**, carrying **156 collision bodies** in all. bf2_tan1 (16 of 16
  doors on arcs) is one of SK's three no-outcome maps, where team 1 fired ZERO shots all round:
  worth looking at next, not a claim.
- A disc quirk found on the way, faithful in the port already: on `pol1_prop_door`,
  `kam_bldg_exterior_door`, `kam_bldg_podroom_door` and `dea1_prop_garbage_room_door` the
  `OrdnanceCollision = p_door` line names a body the model does NOT have (the boxes are
  `p_door2_1`, `p_door1`, `p_-so-rob` …), so by `re-collision-layers.md` §2 the ordnance subset
  empties and bolts pass through those doors — on the disc exactly as in the port. [V][O]
  **Corrected 2026-09-23 (cleanse): WRONG on both sides.** A name miss writes NOTHING: `OrdnanceCollision` is
  `FUN_00216288` (`re-collision-layers.md` :212) and at BF2 decomp 128707-128710 *(decompiler detail omitted)*
  skips the accumulator write; the port does the same (`game/src/collision_layers.rs` `layer_bodies`, "no write at
  all", pinned by `a_line_naming_an_unknown_body_changes_nothing`). The ordnance subset keeps its MASK-seeded
  bodies, so bolts STOP at these doors on the disc and in the port.

## 6. The BF1 twin (§5 self-check 3) and the absence searches (self-check 2)

- **BF1 has no door ENTITY CLASS** — it does ship door-shaped props, which is a different thing.
  Three independent searches: (1) `extracted/bf1/SWBFRONT.ELF` (4,238,908 bytes) contains **zero**
  case-insensitive occurrences of the bytes `door` — no label, no RTTI string (BF2's ELF: 5);
  (2) BF1's class-label factory is `FUN_00172ab0`, and its table — 35 labels at VAs
  0x00527110–0x00527340 — runs `… armedbuilding, armedbuildingdynamic, **trap**, soldier, droid,
  walker …`, i.e. it goes straight from `armedbuildingdynamic` to `trap` exactly where BF2's goes
  `armedbuildingdynamic → door → trap`; (3) over all **120 BF1 LVLs, 0 of 788 top-level `entc`
  classes** declare `BASE = "door"`, against **26 of 1347 over the 165 BF2 LVLs** (15 distinct
  classes, including `mus1_prop_window`, which is a door). Counting `entc` nested inside `lvl_`
  sub-containers as well gives BF1 **908 / 0** and BF2 **2163 / 28** — the absence survives either
  way, and `extracted/bf1/lvl/DATA/_LVL_PS2/TEST/TESTH.LVL`, which is a ZIP rather than a ucfb,
  was decompressed and searched too (0 hits). What BF1 DOES ship is 201 case-insensitive `door`
  byte hits across 12 LVLs and **6 door-named entity classes — every one of them `BASE = prop`**
  (`nab2_prop_door`, `Rhn1_prop_ruin_door_01/_02`, `yav2_bldg_door_ruin_8` …): scenery, with no
  engine behaviour behind it. [V][O] (`base_census.py`, and the verifier agent's own walk)
- **The port has no door behaviour**: `grep -rn 'door' --include=*.rs engine/crates/` returns 7
  lines, all comments or unrelated identifiers (`gallery.rs`'s doorway example, `walk.rs`'s "room
  next door", `mission.rs`'s Tantive `blastdoor` region name). [V][O]
  **Corrected 2026-09-23 (cleanse):** 8 at `2d5e4a4` — the eighth,
  `core/src/ai.rs` `fill_detour`'s "a BF2 door the mission Lua opens", was exactly the reading this RE refutes (its
  comment is fixed in the cleanse's code commit).
- **The port never applies `plnp`**: `grep -rn 'plnp' engine/crates/{core,game}/src` finds one doc
  comment in `core/src/nav.rs` and nothing else; `bake` writes the sidecars and `mungefs::plan`
  says in its header that it "does not apply" the patches. [V][O]

## 7. What the port owes (queue SW's follow-ons)

- **DR1 [SWEEP] — the cheap, reversible unstick:** the bodies of a `base: "door"` instance leave
  the SOLDIER subset behind a flag (`OPENBF_DOORS_SOLID`, default off = open), a declared
  deviation while there is no door entity. It is the disc's steady state for an unlocked door
  with anyone nearby, and it costs 156 bodies out of 26,980 instance-bodies. Re-baselines the
  sweep and the replay hashes (declared).
- **DR2 — the door entity itself:** the trigger sphere on the `AnimationTrigger` hardpoint, the
  `open` clip driving the prim's transform, `IsLocked`, the two sounds. Needs a moving body in
  `CollisionWorld` (today's broadphase is built once) and prop animation playback, which nothing
  in the port has yet.
- **DR3 — `plnp` + `Block/UnblockPlanningGraphArcs`:** only worth it for the campaign scripts and
  the locked-door case; SP conquest on these two maps asks for the all-open table the port
  already uses.

## 8. Not closed ([H], for a later pass)

- The name behind PROP key **0xa1c9cbc5** (`"metal_foley"`) — not in my candidate set. [H]
- **Which** vtable slot of the portal object `FUN_005beba8` hands out is being toggled, and what
  the portal does with it (visibility is the guess). [H]
- Whether the PS2's compiled MISSION.LVL Lua matches the PC modtools source used in §4. [H]
- The door's own query mask in `FUN_002b0260` (the `7` and the `0x14` cap) was not traced past the
  call, and the gated `FUN_003f0990` alternative not at all. [D]
- **Added 2026-09-25 by DR2's build (lane 1), which ports this file's rule (`core::door`, `bake::door`):**
  WHICH position of a soldier the radius query measures is unread; the port uses the feet. Under that
  reading two DEA1 classes never open [H]: `dea1_prop_door`'s trigger is `hp_door` r = 1.0 at model
  (0, 1, 0) — 1 m above the floor, so feet are never strictly inside — and `dea1_prop_garbage_room_door`'s
  clip moves none of its prims (its trigger `hp_exittrigger` r = 1.0 sits at model (27.0, −6.8, 102.2)).
  Whether the disc means them to open is open; the baked rigs are in `bf2_dea1/manifest.ron`. [H][O]

## 9. How this was verified (`OPUS_LOOP_PROMPT.md` §5)

**The four self-checks.** (1) Every decomp citation names its enclosing function — which caught a
real error: the vtable at 0x61b508, tempting because it sits in the door's neighbourhood, is
**EntityDroid's**. (2) Every absence claim was searched, not read: §6's three BF1 searches, the
port's `door` grep and the `plnp` grep, all with counts. (3) The twin was opened: BF1's own
class-label factory `FUN_00172ab0` was read label by label, and BF1's door-named classes were
enumerated and found to be `prop`s. (4) Constants were read as BYTES — the label at 0x60fb68 and
the `"%s %f"` at 0x61afc0 from the ELF, the trigger radii, the rig, the prim's `PRNT` and the
clip's keys from the raw LVLs.

**One Opus verifier agent** was given the 14 claims and their citations only — never the reasoning
— and asked to open each cited line, name the enclosing function, and try to falsify both absence
claims with its own searches. It **supported every load-bearing claim** (the label bytes, the six
property hashes and their slots, all eight fnv1a preimages, the tick and its thunk, the trigger
loop, the skeleton, the prim's parent, the clip CRC and the decoded travel, the Lua lines and the
tree-wide counts, and all three BF1 searches, which it re-ran and could not break — it even found
and decompressed a BF1 LVL that is a ZIP). It **falsified or tightened six wordings**, every one of
them folded in above before this file was written:

1. `EntityDoor`'s name was argued from vtable adjacency; there is a zero word in between. Replaced
   with the module's static init `FUN_001ec950` (hop 3).
2. The two sounds are **edge-triggered by latch bit 0x21**, not played while the flag holds, and
   the locked arm needs the trigger flag as well (hop 5).
3. The trigger loop has a second, gated query path `FUN_003f0990` (hop 5).
4. "BF1 has no door at all" → **no door ENTITY CLASS**: BF1 ships 6 door-named classes, all
   `BASE = prop`, and 201 `door` byte hits across 12 LVLs (§6).
5. "the Lua only locks a door" → POL1's conquest scripts DO close a doorway, by `PlayAnimation` on
   `Pol_prop_airlock_door`, whose BASE is `prop` (§4) — two mechanisms, not one.
6. Small exactness: the handler ends at line 99967; frame 0 of the clip is (0, −0, 0.0001); the
   BF1 label table is 35 labels to 0x527340; 180/178 are occurrences, not calls.

It also strengthened two claims: `FUN_001ec5d0`'s address occurs exactly ONCE in the ELF (slot
+0x20 of the class vtable 0x61afc8), and `FUN_001ebb80` has no pointer anywhere — the thunk at
0x61b004 is its only entry — so "this is the door class's handler / the door's tick" is proven
rather than inferred. Its scratch is under `lead_tmp/re/stuck-wall/verify/`.

## 10. Where a door's bone-bound geometry rests (card STDOR, 2026-10-01; closes card BNR) [D + PS2 frames]

**Answer.** A door model's `BNAM` segments and its `PRNT` collision prims are stored in their bone's frame, like every
other `BNAM` segment on the disc (card TBIND, `bake::bind_pose`). The port had read both as model space. No ELF hop
was read for this: the evidence is the disc's own data and PCSX2 frames, so every line here is [D], not [V].

**The disc's data** (`DEA1.LVL`, `MUS1.LVL`: `skel`, `modl`, `prim`, the `open` clip's first and last keys):

| class | bone bind | stored geometry | through the bone |
|---|---|---|---|
| `dea1_prop_door_blast` | `bone_leftdoors` x −5, `bone_rightdoors` x +5, no rotation; clip −5 → −10 and +5 → +10 | left leaf x 0.74..7.36, right −7.36..−0.74: outside the model's own frame (x ±6), a 1.48 m slit | left −4.26..2.36, right −2.36..4.26: inside the frame, meeting on a diagonal seam; open, clear of x ±2.64 |
| `mus1_prop_door_tri` | `bone_door` rotated 20.8° about x; clip travel (0, 3.707, 1.409) = 3.965 m along the bone's own y | an upright panel, which the travel would carry 1.4 m out of its plane | a panel tilted 20.8° that slides in its plane |
| `mus1_prop_door_garage` | `boneroot` rotated 20.8° about z; travel (1.218, 3.221, 0) = the bone's own y | upright | tilted, slides in its plane |
| `mus1_prop_window` | panes' bones at (0.514, 1.445, 0), all three bones tilted 20.6° | panes y 0..3.24 at x 0, shield y 1.18..4.34 at x 0.85 | panes and shield in one slanted plane, y 1.40..4.47 |

Every other door class (KAM1, POL1, TAN1, DEA1's small door) binds its bones at the identity, where both readings are
the same bytes.

**PS2 frames** (PCSX2, pairs in `engine/artifacts/lead_tmp/stdor/pics/`; port renders at the PS2's pose):
- DEA1 blast door 2 from 22 m, outside its 16 m trigger (`ref/bf2_dea1/walk/door2_far_b000.png`): shut, no slit,
  diagonal seams. Model-space port: a 1.3 m slit. Bone-frame port: shut, the same seams
  (`sheet_bf2_dea1_door2_far_b000.png`).
- The same door from 9 m, caught opening (`door2_near_b000.png`): a diamond-shaped hole at the middle of the seam.
  Bone-frame port at `open` 0.15 / 0.25 / 0.35 draws the same diamond growing (`sheet_dea1_door2_opening.png`);
  the model-space port at `open` 1 has both leaves across the doorway.
- MUS1 garage door from 12 m (`ref/bf2_mus1/walk/cp5_cp6_b180.png`) and square door from 14 m (`cp2_cp3_b090.png`):
  the panel fills its slanted frame, under the frame's header. Model-space port: an upright panel standing in front
  of the frame, a gap over it. Bone-frame port: as the PS2 (`crop_cp5_cp6_b180.png`, `crop_cp2_cp3_b090.png`).

**Built** (`bake::bind_pose`): door models go through `pose_bnam` like every other model; `BindPose::rest_prims`
puts a door geometry's prims through the same bind matrices. `bake::door`'s motion tables (`B(f)·B(0)⁻¹`) are
unchanged. A scratch bake of every map differs from the old one in 7 files: `bf2_dea1` manifest + blast door GLB,
`bf2_mus1` manifest + its four door GLBs.

**Still open.** (1) Prims of geometries that are NOT doors are still read as model space (`mungefs::coll`'s
PROVISIONAL); TBIND's note says DEA1's trash compactor prims share their segments' bone frame. (2) The door clip's
frame 0 is taken to be the bind (largest difference read: 0.3 mm on MUS1's garage door).

## 11. What the PS2's bots do at a door that is opening (card MOUTH2, 2026-10-12) [L]

Measured, not read from code: two 8-minute captures of BF2 Kamino conquest at 10 Hz, 16 AI a side, the player
parked 260 m off (`engine/artifacts/lead_tmp/mouth/ps2/bf2_kam1_m{2,3}_units.csv`; card MOUTH's sampler: feet, the
stick, the contact push E+0x4c0, the path follower's nodes). The tool is
`engine/artifacts/lead_tmp/mouth2/tools/doorpass.py --rows --ps2 <csv> ...`: it rebuilds the door's parameter from
every unit's feet by §2 hop 5's rule and prints each pass through the pod room door - the `kam_bldg_podroom_door`
instance named `open` at (91.475, 51.592, 180.442): trigger `hp_door` r 4.0, 41 frames (1.333 s), two leaves that
slide 3.75 m apart along (0.9988, 0.048), so a 7.5 m doorway.

- **44 passes, all 44 northbound** (into the pod room), on lanes from 3.18 m west to 2.99 m east of the doorway's
  centre: 17 within 2 m of it, 27 further out (`engine/artifacts/lead_tmp/mouth2/out/ps2_doorpass.txt`). The
  follower's nodes there are each bot's own (hub points, and for some two corner nodes 5 m apart down the
  doorway): the lanes are spread, not gathered on the centre.
- **A bot on an inner lane is through at speed.** 0 of the 17 slowed under 2 m/s: in at a median 6.8 m/s, at
  the plane 0.47-0.73 s after entering the trigger with the door about half open (the leaf's edge 2.5-3.1 m from
  the centre), slowest half second a median 4.8 m/s.
- **A bot on an outer lane walks up to the leaf and stands.** 20 of the 27 slowed under 2 m/s, 19 of them at the
  leaf (lanes 2.46-3.18 m off the centre; the 20th stood 14 s in the trigger, not sampled further). Each stops
  with its feet 1.32 m short of the door's plane (k 32 at t 65.2-65.8, k 38 at 151.9-152.5 in `m2`): the contact
  push reads (0.048, -0.999) - the door's normal - on alternate samples, the stick reads 1.000 and then 0.429,
  the mover's aim point alternates between a point just ahead and one 5 m along the leaf (the contact rule of
  `re-soldier-ai.md` 2: the unit's own position + 5 m along the perpendicular while the push is set). The feet
  move under 6 cm. When the leaf has slid past the lane (rebuilt parameter 0.73-1.00) the bot walks through:
  0.88-1.49 s inside the trigger before the plane.
- **By card MOUTH's stall rule** (1.5 s of forward stick with under 1.5 m gained) the door has 2 stalls in 51 passes
  of its 10 m disc, one backed up.

**The port before** (main 5fbbbd4, 8 seeds x 3 min, `engine/artifacts/lead_tmp/mouth2/snap_main`): 121 stalls in
270 passes, 116 backed up. The steering's sweep met the leaf 0.3 m ahead, the avoidance committed to a side and the
bot followed the wall away from the door for the round's 10 s allowance - 30-50 m - re-planned and came back.

**Built** (card MOUTH2, main ac13ca6): `ai::steer`'s `StopKind::Door` - the sweep's answer when the body it met is
a leaf of a door that is opening (`Sim::door_opening`: a living unit in a trigger, the clip not at its end) and
the other probes find the way clear; the avoidance walks on at its point and the wall pass holds the bot at the
leaf until it clears. The mechanism is the port's (its sweep is its own, `ai.rs`'s header); the result is the PS2's
stand-and-go. With it: that door 0 stalls in 157 passes; stalls beginning within 6 m of any door
(`engine/artifacts/lead_tmp/mouth2/tools/doorsites.sh`) on Kamino 101 -> 1, Death Star 61 -> 29, Polis Massa
38 -> 31, Mustafar 98 -> 83, Tantive IV 315 -> 257.

**Open.** (1) What still stalls at Tantive IV's and Mustafar's doors is not a leaf (untraced). (2) The Death
Star's blast doors take 5.0 s (151 frames, trigger r 16): a bot waits out up to 2.7 s of it at the leaf; the PS2's
wait there is not sampled. (3) The conquest script locks Kamino's pod room doors 32 and 28 (§4); the two PS2
captures hold 0 bot samples within 2 m of either and the port's 24 snapshots 0, so the then-missing lock moved 0 bots in those samples. Startup locks and locked-door audio have since landed
(public `40535bf`), with original-game audio comparison pending. — (1) is traced in §12.4 (card DOORS); (2) the port's own wait there is in §12.3.

## 12. Every door of the five door maps, censused (card DOORS, 2026-10-12) [L]

Measured, not read from code. The tools are `engine/artifacts/lead_tmp/doors/tools/` (README there): `census.py`
(per door: the route bot-minutes in a 6 m disc, the visits, card MOUTH's stall, direction reversals on the 0.5 s
grid - one rule on a port black-box snapshot and on a PS2 capture), `classify.py` (what each stall is, from the
black box's rows), `waits.py` (every meeting of a bot with a leaf). Port: 8 seeds x 3 minutes a map.

**12.1 The PS2's own doors, BF2 Kamino** (the three captures of §11 and card MOUTH, 153 route bot-minutes): 10 of the
12 doors are walked, 133 visits come within 3 m of a door, 125 go through, 1 turns back, **5 stalls** (the exterior
door `fdl-1` 3 in 48, the pod room door `open` 2 in 38: one bot each) - 4 per 100 near visits, 17.4 per 20 door
bot-minutes, 41.7 reversals per 20. The PS2's bots are not flawless at doors; they do not turn away from them.

**12.2 The port after card MOUTH2** (main 4dbb460):

| map | doors walked | near visits | door stalls | per 100 near | per 20 door bot-min | reversals per 20 |
|---|---|---|---|---|---|---|
| BF2 Kamino | 10 | 579 | 1 | 0 | 1.0 | 29.9 |
| BF2 Death Star | 4 | 459 | 22 | 5 | 29.1 | 38.4 |
| BF2 Polis Massa | 7 | 1050 | 41 | 4 | 21.3 | 14.0 |
| BF2 Mustafar | 22 | 1823 | 85 | 5 | 28.9 | 19.0 |
| BF2 Tantive IV | 14 | 1713 | 192 | 12 | 60.7 | 45.0 |

Of the 358 stall rows near a door **6 are at a leaf** (the Death Star's, 12.3), 107 are a bot going round another
body on a committed side, 172 are a bot STANDING IN A FIREFIGHT at the doorway (its walk flag flickers between
target losses; a doorway is where the teams meet), 45 are moving in one, 28 are other. Of 784 meetings of a bot with
a leaf 773 are through, or still at it, 3 s on; the wait's median is 0.5 s and its longest 2.0 s; 0 stuck events and
1 failed progress window fall in them. So §11's rule needs no exemption from the stuck windows, and a walk down the
opening's centre line (the maintainer's ask 1) is not what the passes lack.

**12.3 The Death Star's blast door** (`dea1_prop_door_blast`: a 16 m trigger, 151 frames = 5.0 s). Its two leaves
part on a DIAGONAL: the bake poses a leaf as boxes rolled 43.9 degrees about the door's normal (half extents 1.26 x
3.50 x 0.74 and 1.32 x 3.60 x 0.20, and one upright 1.71 x 2.57 x 0.74), each sliding 5 m. The edge that leaves the
doorway last is therefore a face at n.y 0.69 rising from the sill. The port's steep-floor probe (card STHOP: a ray
down a metre ahead of the feet) lands on it - over the feet, under the control limit - and its answer outranked the
door's: seed 62 unit 31 at the Empire's spawn, every pick printed, reads `Door` at tick 68 (the door 39 % open),
`Steep` at tick 71, a side committed, 22 m along the hangar wall. On main that unit commits a side there in the
first 5 s of 31 of 32 rounds.

**Built** (card DOORS, main 4aaa2a2): a steep face found in the box of a leaf of a door that is opening
(`Sim::opening_leaf_over`) is no steep floor. Main bc10b82 against it, the Death Star over 32 seeds x 3 minutes:
meetings that end with the bot 2 m further from its point 18 of 364 -> 5 of 374; the spawn's unit 31 commits a side
in 0 of 32 rounds; door stalls 72 -> 48 (28.9 -> 19.1 per 20 door bot-minutes); reversals within 6 m of a door 102 ->
43; the longest wait at a leaf 2.0 s. Tantive IV's and Mustafar's tables do not move, Kamino's by one bot (3 -> 4
stalls in 2,350 near visits).

**12.4 What is left near doors is not the door** (card CORN). Tantive IV's worst door (`rpodroom1`, 45 stalls in 149
near visits) is the leg from hub 20 west to hub 79, which does not cross it: the corner search answers with the
corner (-278.9, 151.7), a point INSIDE `barrier33` - the wall north of the corridor - because the bot's point of hub
79 lies 0.90 m outside that barrier, inside the search's 1.0 m radius, so the barrier is ignored for the whole leg
(`NavGraph::leg_ignores`) and the chain round `barrier219` goes through it. The bot meets the corridor's wall,
commits a side and walks 22 m round through the door beside it. A first rule ("a corner inside a barrier the leg
ignores is no node") takes Tantive IV's door stalls 192 -> 140 but is no gain over all 32 maps (S-stalls 2566 ->
2581, Utapau 391 -> 471): not taken. And a hub point or a corner can lie behind a door's jamb (Tantive IV's tech-room
doors, Kamino's `podroom_door24`, the Death Star's leg 4 > 1).

**Open.** (1) The PS2's bots at the Death Star's blast door are unsampled: the captures on disk with the follower's
columns are Kamino's. (2) The PS2's containment test for an ignored barrier uses the unit radius alone (the port
adds 0.5 m): which of Tantive IV's corners the PS2's own search hands out is unread.
