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
  uses the newest contact's unflattened world tangent at +0x80 and −15 times
  the gun direction at the gun. It measures the span as ten chords and tests
  them against the target. A new wrap stores half the cubic derivative in
  world axes at the hit's fractional position in the chord (316443–316497).
  The stored contact adds that fraction times the unit chord direction
  to the chord start before transforming into the primitive's frame.
  The original general ray wrapper converts the same query's fraction
  to metres by multiplying by the chord length (`FUN_00162f60`,
  2564–2575). The first span uses `InitialCableLength`. Later spans use `CableLength`.
  A hit on a non-tow body breaks the live link.
- `FUN_002669f8` ignores another hit on the current leg. The previous leg
  blocks a reversal. When the previous leg neighbors the current leg, the
  leg opposite the current one also breaks the link.
- `FUN_003248d8` (316950–317096) relaxes the older +0x80 tangents. The newest
  +0x80 tangent retains its magnitude and direction. The function also shifts
  wrap contacts toward their authored local x/z coordinates while keeping
  their local y.
- `FUN_00323178` counts existing records before sweeping (315850–315858).
  The final insert returns with the live link intact (316497–316498).
  On the next update it changes the cable to state 3, calls
  `FUN_00201b08` to clear the link and returns (316106–316139).
  `FUN_00322890` skips drawing the full live span (315323–315338) and
  draws the detached wraps in state 3 (315428). The state-3 branch
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
The same BF1 update stores the contact using the unit chord direction
and hit fraction, and the half derivative in world axes (206691–206762).
The general BF1 ray wrapper `FUN_00162b60` converts the query's fraction
to metres at 2483–2497. Its final insert and release also return on separate updates
(206762–206763, 206776–206808). `FUN_00280c20` omits the full live
span (205697–205712) and draws the detached wraps (205798).

## Original Hoth hook, wrap and trip

Stock PS2 BF2 Conquest with the menu's 0-AI option captured two ordinary
Rebel rear-seat launches that hooked the uncrewed AT-AT. One used
primitive 11. The other attached to primitive 12, recorded a second
contact at primitive 0, then went loose. The live flyer reference
matched the walker and its generation at both hooks. The second
record was visible in the last state-1 sample, with the link
already cleared in that sample. The walker retained full health.

The human changed from pilot to rear seat in flight. An immediate
return-to-pilot press after the shot left the human in the rear
seat until release. The original seat-cycle function checks the selected
seat weapon's active state before switching (`FUN_0024b4d0`, B2:165302–165316).
With the original 16 AI, a human rear gunner and an original AI pilot
also held a moving walker through eight contacts before it went loose,
with both vehicles alive and the target's health unchanged.

A complete stock BF2 trip records one cable, one flyer generation and
one walker generation across 492 consecutive bracketed updates. The
thirteenth contact arrived at relative tick 1,711 with both links
active. They cleared at 1,712 while the walker stayed at 60,000 HP.
The same walker was dead at 1,713 at −60,000 HP. A second stock run
also reached state 3 with 13 records and killed the tracked walker.
Original post-trip damage continues on the dead walker while the
cable remains allocated.

## Implementation evidence

The branch uses the deterministic walker pose sampler and baked primitives.
Both Hoth bakes supply the rear gun settings, all 12 named leg primitives and
their local offsets. The branch fixtures exercise mounted launch, attachment
to a moving target, range and entity cleanup, ring order and the three
ground trip updates.

Two stock BF2 runs supplied the initial anchor, the gun and posed
target per update. Replaying the ground span solver with the four
profiles' baked leg shapes and authored offsets matches the
continuous contact IDs through 261 and 487 updates, ending at six
and twelve contacts respectively. The old solver first diverged
at ticks 2,682 and 1,483. The largest single-update contact offsets
fell from 2.530 m to 0.13 mm and from 2.669 m to 48.36 mm. These
windows use the original motion and cover 13 recorded primitives.
The complete port flight and six upper-body frames are still open.

The last ground contact keeps the link occupied for its update and
omits the full span from drawing. The next update releases the link
and settles the detached wraps. The following update starts damage.
The existing BF1 single dispatch and BF2 source-seat dispatch remain
in force. After the hull is destroyed, the viewer samples
the authored death clip from its final transform. Posed building bodies
block normal movement and ordnance queries until the clip ends, while the
walker has no live vehicle identity. The port removes the cable when
the hull dies. The original's later corpse contacts and their random
draws, death-phase movement and effects, and rear-seat drawing still
need comparison.
