# Snowspeeder tow cable

The BF1 and BF2 Hoth class data give the rear gun a `towcable` ordnance with
a velocity of 250 m/s and both cable length limits at 60 m. The AT-AT declares
12 `TowCableCollision` rows, three per leg in ring order. A row names a
collision primitive and three local coordinates.

## BF2 source reading

- `FUN_00263f68` (183109–183149) resolves the named primitive, adds it to
  the tow subset (selector 0x800), and stores its local coordinates in list
  order. `FUN_002669f8` partitions the list into four equal leg groups.
- `FUN_003223d0` (314887–315130) accepts a head contact on a primitive in
  that subset and records the contact in the primitive's moving frame.
- `FUN_00323178` (316160–316499) builds the live cubic span to the gun. It
  uses the stored tangent at the wrap and −15 times the gun direction at the
  gun, measures the span as ten chords, and then tests each chord against the
  target. The first span uses `InitialCableLength`. Later spans use
  `CableLength`. A hit on a non-tow body breaks the live link.
- `FUN_002669f8` ignores another hit on the current leg. The previous leg
  blocks a reversal. When the previous leg neighbors the current leg, the
  leg opposite the current one also breaks the link.
- `FUN_003248d8` (316950–317096) relaxes the stored tangents and shifts
  wrap contacts toward their authored local x/z coordinates while keeping
  their local y.
- `FUN_00323178` (316109–316143) changes the cable to state 3 once the
  thirteenth record is present and clears the shooter's live link.
  `FUN_00201b08` clears that reference. The state-3 branch
  (315871–316062) picks a prior contact record with the two-step gameplay
  random stream and builds the ordnance packet via `FUN_0030fa88`. It
  dispatches once per occupied source controller while setting the
  friendly-fire scale to one. The commandwalker health table at
  0x62bd38 selects `FUN_002670d0` → `FUN_002620e8` → `FUN_001e0580`,
  the ordinary walker damage path.
- `FUN_0025ce68` (177638–177711) marks the walker dead, moves its
  collider to the building list, and dispatches the opening death effect.
  `FUN_00261208` continues writing posed collision until the animation
  reaches entry 4. `FUN_0025f4a0` (179893–179987) then finalizes removal.
  The two Hoth bakes carry a 261-frame AT-AT `death01` clip.
  BF1 `FUN_00218ac8` likewise re-registers the dying walker as a building.

## BF1 difference

The BF1 tow class is constructed by `FUN_00282758`. Its update
`FUN_002814f8` (206227–206378) samples the prior records with the
same two-step generator shape, but builds one ordinary ordnance
descriptor via `FUN_00272f88` and dispatches it once. There is no
source seat loop or temporary friendly-fire override in that path.

## Implementation evidence

The branch uses the deterministic walker pose sampler and baked primitives.
Both Hoth bakes supply the rear gun settings, all 12 named leg primitives and
their local offsets. The branch fixtures exercise mounted launch, attachment
to a moving target, range and entity cleanup, ring order, and the thirteenth
record threshold. They do not measure the retail trip.

The wrapped cable starts the post-wrap damage on the next tick, after
releasing the active link. After the hull is destroyed, the viewer samples
the authored death clip from its final transform. Posed building bodies
block normal movement and ordnance queries until the clip ends, while the
walker has no live vehicle identity. The release settling effect, native
death-phase movement and effects, the full wrapping sequence, and the
rear-seat drawing still need live comparison.
