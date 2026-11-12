# OPUS RE (2026-10-29) - [O] UNAUDITED
# RE: BF2's destruction chunks as rigid bodies — trails, smoke, terrain hits, node origins (issue #49)

**The question** (issue #49): the port flew a dead vehicle's chunks as spheres with a trail-less bounce. The PS2
flies each one as a rigid body on its model's collision prim, with `ChunkTrailEffect` trails, a
`ChunkSmokeEffect` on rest and a `ChunkTerrainEffect` burst, and starts it at the vehicle's `ChunkNodeName`.
Addresses are BF2 (SWBFRNT2.ELF).

## 1. The template (ODF handler, ~B2:127400–127620)

`ChunkGeometryName` opens a template and sets its defaults:

| Field | Default | Key |
|---|---|---|
| gravity | (0, −9.8, 0) | ChunkGravity (75 BF2 chunks author "0 0 0", mostly space `_sc` classes) |
| +0x94 bounciness | 0.6 | ChunkBounciness |
| +0x98 stickiness | 0.7 | ChunkStickiness |
| +0xa0 death speed | −1 | ChunkDeathSpeed (SIMPLE only) |
| +0xa8 lifetime | −1 | ChunkLifetime |
| +0xe0 linear damping | 0.001 | ChunkLinearDamping |
| +0xe4 angular damping | 0.004 | ChunkAngularDamping |
| +0x58 budget | 0 | ChunkTerrainCollisions (a byte) |
| +0x60 mode | 0 | ChunkPhysics: FULL 0, SIMPLE 1 (and any unknown word), STATIC 2, LEAF 3, 4 the cockpit |
| +0x70 omega | 0 | ChunkOmega, a world-frame angular velocity |

- `ChunkNodeName` (0xebf014fd) stores the node's hash at class +0x138 + slot·4.
- Up to 3 trails and 3 smokes per slot; the counters reset at `ChunkGeometryName`. A `ChunkTrailNodeName` /
  `ChunkSmokeNodeName` names the most recent effect's node.
- `ChunkTrailNodeName` is never authored. `ChunkSmokeNodeName` (`hp_smoke1`…) is authored 68 times.

## 2. The body

- **The prim.** The model loader (FUN_00160380) registers each prim of a model's `prim` chunk as "<model>%d".
  The template takes "<model>0" when there is no "<model>1"; otherwise its prim is a sphere at the model's bbox
  centre, radius the half diagonal.
- **Census.** Every chunk model on both discs has exactly one prim.
  - BF2: 83 spheres, 39 cylinders, 38 boxes.
  - BF1: 69 spheres, 36 cylinders, 2 boxes.
  - DATA type 0 = sphere (r), 2 = cylinder (r, full height), 4 = box (half extents).
- **Inertia per unit mass (FUN_00160b28).**
  - box: ((y²+z²), (x²+z²), (x²+y²)) × 0.33333 on the half extents
  - sphere: 0.4·r²
  - cylinder: Iyy = r²/2, Ixx = Izz = r²/4 + h²/12
- **Mass.** 40 (+0x3d0), inverse 0.025 (+0x3d4). The inverse inertia is 1/(40·I), or 1 when I ≤ 0.
- **Bound radius.** sphere r; box |h|; cylinder √(r² + (h/2)²).
- **Frames.** The body frame is start × prim-local, and the model is drawn at body × prim-local⁻¹.

## 3. The launch (FUN_00213630) and init (FUN_001d5b00)

- **Start.** The vehicle's matrix (+0x48..+0x54) times the `ChunkNodeName` node's matrix, when the node is found.
  - The node table comes from the vehicle's vtable +0x4c (else +0x48 fills a local one). It is a hash → matrix map
    looked up by FUN_00539150 (open addressing, 0x80 slots).
  - Its keys are the model's skeleton bones; node names are kept on the disc only as `skel` strings.
  - Census: of the authored `ChunkNodeName`s, only the hailfire's `wheel_left` / `wheel_right` (both games) name a
    bone. `Wing_gun_housing_2` and `main_backdoor_R` (gunships, X-wing, landspeeder) appear nowhere but the ODF
    PROPs, so their lookup fails and the chunk starts at the vehicle's matrix.
  - The spider's chunks name `cis_walk_spider`, which is the model's own NAME, not a bone. Whether the table keys
    it is not traced; its root sits at the model origin either way.
- **Velocity.**
  - The direction is from the cluster's centroid, with ChunkUpFactor × the distance added to y.
  - ChunkSpeed ≥ 0: parent velocity × inherit + direction × speed.
  - ChunkSpeed < 0: U(15, 30) / r along the direction (r = the vehicle's bound radius) when the squared distance
    is > 1. LEAF scales the result by 0.25 before the check below.
  - If |v| < 5 after that: v = direction × U(4, 6), with y raised to 2 + U(0, 1) when below 2.
- **Spin.** L = I_w·Ω and ω = Ω, where Ω is ChunkOmega as authored, carried through the body frame. Read live on a
  frigate's pieces (`re-space-assault.md` §23): the template holds the authored vector and the piece holds body frame × vector,
  with no sign change.
- **STATIC chunks** start with count 1 and budget 0, at rest.
- **Trails** start at spawn (FUN_001d6368).
- **The pool.** 8 clusters × 10 chunks, reused round-robin.

## 4. The tick (FUN_001d6608)

1. **The lifetime timer.** It runs when the chunk is at rest, when the map has no terrain, or when the budget is
   0xff. The life is:
   - 53 s for modes 2 and 4;
   - otherwise 8 s on BF2 and 3 s on BF1;
   - ChunkLifetime when it is ≥ 0;
   - 30 s for budget 0xff.

   The chunk is killed when the timer reaches its life.
2. **Integration.**
   - The +0x370 contact flag resets.
   - F = m·g − c_lin·v.
   - Modes other than 1 and 4: L −= c_ang·L·dt, then ω = I_w⁻¹·L.
   - The body rotates by ω·dt when |ω·dt| > 0.0001.
   - v += F/m·dt, then pos += v·dt.
3. **Settling.** Only when count > budget.
   - It casts a down ray of length r·1.001 against terrain and buildings (mask 0x90). It settles when the hit
     fraction is < 0.75 (0.3 for mode 4).
   - It also settles when the mode is 2, or when count > budget + 5.
   - Settling clears `moving` and starts the smoke (FUN_001d64b8). An explosion cluster sets its timer to 3.0.
4. **Below the terrain** (gated by DAT_0060bed4, "terrain loaded"). The test uses the integrated position: when
   pos.y + r < the terrain height and a 256 m down ray finds no building (0x10), the chunk dies (FUN_001d5850).
   FUN_00161c60 returns the hit fraction (1.0 = none).

## 5. The FULL contact (FUN_001d7740)

B2 lines 84822–85112 (`SWBFRNT2.ELF-decomp.c`):
- Layer checks (84867–84879):
  - Layer 2 (Soldier): returns at once; chunks ignore soldiers.
  - Layer 8 (Vehicle) and 0x10 (Building): a ghosted object (its +0x50 test) is skipped; live vehicle hulls and
    building bodies are contact targets.
- A chunk takes one impulse per tick (the +0x370 flag).
- At the contact point: vp = v + ω×r, vn = vp·n, t = −unit(vp − n·vn).
- **Over budget on terrain** (84938–84957). When count (+0x461) has reached `ChunkTerrainCollisions` (+0x460) and the
  contact is not a vehicle (8) or a building (0x10):
  - when count == budget, play ChunkTerrainEffect at (p.x, terrain height, p.z) and count + 1;
  - either way, return with no impulse and no push-out.
  Vehicles and buildings are left out of this test, so they always bounce a chunk.
- **Otherwise:**
  - j = (−e·vn − vn) / (1/m + ((I⁻¹(r×n))×r)·n)
  - v += n·j/m; L += r×(n·j); ω = I⁻¹L
  - Friction acts on L only, when vt = (v + ω×r)·t < 0:
    - jf = −vt·μ / (1/m + ((I⁻¹(r×t))×r)·t)
    - L += r×(t·jf)
  - Then count + 1, and the generic FUN_00169668 pushes the body out by the depth along the normal.
- The SIMPLE handler (FUN_001d7250) is a linear-only bounce with the ChunkDeathSpeed kill. No chunk authors SIMPLE.

## 6. Trails and smoke

- A trail with no node rides the body frame; with a node, the model × node.
- The smoke rides model × node (identity with no node).
- Killing a chunk (FUN_001d5850) stops both.

## 7. The port

`game::fx::chunks` runs §2–§6.
- The bake writes chunk models' prims (vehicles.ron `collision`) and their skeleton hardpoints (`geometries[].hardpoints`).
- `vehicle_chunks` (example, removed in the sprint's code cleanse; in git history) captured a pad vehicle's death on GEO1. The fighter tank shows its trails, its
  `dirtspray` bursts after its budget, and the `hp_smoke1` plume of the settled piece.

**[ours]**
- Contacts come from the terrain, the map's static bodies, and live vehicle hulls: `Layers::SOLDIER | Layers::VEHICLE` with `HullFilter::All` queried through `sphere_contacts_filtered` / `shape_contacts_filtered`. Per the PS2 trace, bodies and live vehicle hulls are excluded from the terrain collision budget suppression and receive bounce impulse. Chunks never damage units.
- Ground modes keep the PS2 floor rays: the settle ray and the below-terrain ray see the map's bodies only
  (`HullFilter::Static`; §4's masks 0x90 and 0x10 hold no vehicle), so a piece never rests on a hull that drives
  away. Space rounds cast them at every hull.
- A class with no chunk slot of its own throws nothing on the ground. In space it borrows its explosion class's
  slot, `NumChunks` copies (at most 10), on the vehicle law. The explosion's own debris law (`re-vehicle-death.md`
  §6.1: random direction, ChunkSpeed as a range, no bounce, 5 s) is not ported; `replace:` both with that law.
- The rest life is 3 s on the ground on both games. A space round keeps BF2's 8 s, measured on a frigate (`re-space-assault.md` §23).
- SIMPLE uses the FULL handler, since SIMPLE is never authored.
- The push-out ignores the chunk's own collision layer, which is not traced.
- Historical before issue #80: droid chunks used mesh-bounds spheres and ignored `ChunkNodeName`.
  The targeted droid bake now records SBD prims and launch nodes; battle rendering uses the last drawn
  soldier pose at those bones, with bind-pose fallback for callers without soldier visuals.
  Magnaguard cuts and the cross-game SBD profile are [ours] (FEEL_NOTES, 2026-10-29).

**Observed.** A parked hailfire's hull (`hailfire_chunk3`, ChunkSpeed 20, UpFactor 0) starts 0.66 m below the
vehicle origin. That origin is itself under the ground: 0.148 m on the PS2 (RE_LOG, the hover rest census) and
0.18 m in the port. The centroid direction points down, so §4 removes the hull on its first tick in the port.
The law reads the same on the PS2, but this is not checked against PS2 footage.

## 8. The draw (FUN_001d7cb0)

Slot +0x54 of the cluster vtable at `0x6192c8` is `FUN_001d85a8` (B2:84905-84922), which walks the ten pieces and calls
`FUN_001d7cb0` (B2:84462-84684) for each live one.

- **The model call.** It ends in slot +0xc of the model's vtable at +0xc0 (B2:84681-84682), with the piece's matrix, a pose block,
  the colour and the caller's flags with bit 8 set. `FUN_001c5610`, the ordinary object draw, reaches the same slot (B2:71417). In
  save state 10, 319 objects hold vtable `0x64feb0`, whose slot +0xc is `FUN_0057cf60` (B2:700365). A chunk's own vtable was not read.
- **Lighting.** `FUN_0057cf60` gathers up to four lights for the model's sphere unless bit 0x10 or the top bit is set, and sets 0x10
  itself when `FUN_0057f8e8` returns 0 (B2:700497-700526). The chunk adds only bit 8, so it is lit like any other model.
- **The fade.** The colour is white. Its alpha is the caller's float, multiplied by `0.5 * (life - timer)` in the last two seconds
  (B2:84515-84535). `life` is 53 for modes 2 and 4, otherwise 8, then `ChunkLifetime` when it is at least 0, then 30 when the budget
  byte is 0xff.

The port drew chunks through the effects' `SolidMesh`, which keeps only positions and UVs, so a ship's pieces came out unshaded
beside the shaded ship. `VehicleVisuals::load_chunks` now uploads each chunk model with the ship's upload path, and
`chunk_draws` submits them with `ClassVis::draws`, as the ship is. `Piece::opacity` is the fade, on the port's `life()`.
Droid chunks (`side_<game>/droid_chunks`) are loaded through `VehicleSides::load`, checking `weapons/` for `.glb` models and
falling back to `side_<game>/textures` for textures, allowing droid debris to be shaded via the ordinary lit model path
instead of flat `SolidMesh`.

`trace:` bit 8, which goes on to `FUN_0058ed70` (B2:700609, 700659, 700691), and the flags and float that `FUN_001d85a8`'s caller passes in. The
port takes the float as 1. `trace:` the interpolation branch on +0xc9 (B2:84544-84679); the port draws the tick's pose.

`measure:` no PS2 frame with chunks in view has been compared. A save state a second after a crash would give one. The basis
for the look is the code above and the user's comparison of pieces against the intact ship.

`replace:` droid chunks stay on `SolidMesh`. Their models live in `side/droid_chunks/weapons`, outside `VehicleSides`, so `find_model`
does not find them. The fade applies to them there.
