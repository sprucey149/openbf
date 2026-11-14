# Shared flyer controller default and Geonosis Starfighter spawns

2026-11-14, source `f4936755a270` plus this change, `feature/vehicle-parity`.
Scope: the maintainer reported the BF1 Geonosian Starfighter steering on the
right stick and required left-stick movement as the default for every vehicle.
They also questioned whether the craft spawned naturally on Geonosis: Spire.
No issue was opened and no public ownership was changed.

## Controller rule and implementation

Classic keeps movement on the left on every source map. Hover-capable aircraft
whose active flight model supports lateral translation, with authored
`MinSpeed < 0`, `MidSpeed == 0` and `StrafeSpeed > 0`, translate
sideways/forward on left X/Y and aim yaw/pitch on right X/Y. Continuously flying
aircraft steer yaw/pitch on left X/Y and roll/throttle on right X/Y. Selection
uses supported motion capabilities, with no map or class-name exceptions.
The BF1 model implements continuous strafe; BF2 interprets that axis as roll
(`core/src/vehicle/flyer.rs:1074`), so unused BF2 StrafeSpeed properties do not
select a translation layout.
RC Mode 2 remains an explicit profile choice (left yaw/throttle, right roll/pitch).

The first fix removed the BF1-wide unswapped table and applied the fighter swap
to every flyer. The maintainer then reported LAAT/i and MAF movement on the right.
That implementation confused steering with translation and its all-class test
asserted the same channels everywhere instead of the appropriate motion. It
was corrected before refreshing the packed build. `WalkSim::map_flight_sticks`
now selects the appropriate Classic layout before keyboard/mouse merge.
The generic BF1 mapping claim was also narrowed: the existing MAF table evidence
does not prove every BF1 aircraft uses the same physical layout.
Ground vehicles, passenger/gunner input, bots and normalized simulation pads
retain their existing paths; no physics or spawn behavior was edited.

## Natural spawns

The latest snapshot was read first: `engine/artifacts/playtest/last.json`,
BF1 Geonosis Conquest, exit tick 5067, clean build `f4936755a270`. Its last
60 seconds contain the player on foot, so they do not directly reproduce the
stick input. The final world contains one parked Geonosian Starfighter at
(32.41, -48.87, -123.93) and another being piloted by bot 4. Their class is
`cis_fly_geofighter`.

A fresh mission created through the ordinary battle fixture, without the CLI
vehicle shortcut and without AI boarding, naturally creates two Starfighters:

| pad | parked root (metres) | control post |
| --- | --- | --- |
| geofighter | (32.405, -48.867687, -123.933) | CP6, CIS-owned |
| geofighter1 | (29.332, -48.867687, -118.877) | CP6, CIS-owned |

They are inside the Spire hangar. Both authored pads name the CIS Starfighter
and a 45-second replacement spawn time; initial countdown is zero. Both live
vehicles retain their natural pad provenance, are landed and have empty seats.
No spawn behavior or position was changed. A headless real-map render showed
the two craft at these pads; two useful camera views contain 14,194 and 9,958
vehicle pixels compared with the same world without vehicle draws. Captures
are private under `engine/artifacts/starfighter-controls/natural-pads-*.png`;
views 0 and 1 were inspected and show both craft in the hangar. View 2 starts
behind the hangar geometry and view 3 is occluded, so they are not visibility
acceptance witnesses.

The maintainer's current BF1 slot 5 was read without changing it (SHA-256
`60e7cb032b5c876b1783283acc7715dce6bd4ffc6ace89263901caf0ce1b42d8`).
Its screenshot shows a piloted Geonosian Starfighter flying outside the hangar.
It was not loaded into an emulator for this task. The desired left-stick behavior is a maintainer requirement. No new original-game
control comparison was performed for the three aircraft.

## Checks and limits

- `cargo test --manifest-path engine/Cargo.toml -p input --lib`: 27 passed,
  including the default axes, full-range/deadzone/remapping controls and RC Mode 2.
- `cargo test --manifest-path engine/Cargo.toml -p game --test suite
  flyer_controls -- --nocapture --test-threads=1`: exercises all four physical
  controller axes through layout selection, keyboard/mouse merge, the human
  seat adapter and real BF1/BF2 class physics. The Geonosian class also checks
  actual heading, pitch and speed changes. LAAT/i and MAF additionally check
  actual forward, reverse and sideways velocity with the left stick, no pitch/yaw
  from that translation, and actual pitch/yaw from right-stick aim. The census
  selects translation for BF1 MAF/LAAT; BF2 aircraft retain roll on that channel. The natural spawn witness uses
  the fresh mission, not direct vehicle construction. Two passed and the GPU
  witness was ignored in this CPU invocation; the census covered 15 BF1 and
  30 BF2 classes, including map-local flyers.
- `scripts/gpu_tests.sh --game natural_geofighters_are_visible_at_the_spire_spawn_pads`:
  1 passed, headless and serial; both hangar captures were viewed.
- `scripts/gpu_tests.sh --game-lib
  rc_flight_controls_follow_the_preference_in_ground_and_space_pilot_seats`:
  1 passed, covering pilot selection, passenger exclusion and the opt-in layout
  in BF1 ground, BF2 ground and BF2 space contexts.

The maintainer accepted the corrected controls in the refreshed dist build on
2026-11-14 and authorized the commit. This task does not establish dynamic original-game flight parity, all-map spawn acceptance,
a new performance measurement or an alignment gate result.


## Packed build follow-up

The maintainer then launched `dist/openbf.exe` and found the hangar empty.
Its actual snapshot is `dist/openbf_data/playtest/last.json` (not the checkout's
playtest directory): exit tick 1226 / 40.87 s, seed 440, BF1 Geonosis Conquest.
It identifies executable build `d2a55a1be94a` at 2026-11-14 18:00:48 local time,
so the packed executable predated the Hailfire and shared-controller fixes.

The same snapshot also settles the apparent missing spawn: by tick 60 both
Starfighters already exist and are taking off with bots 32 and 33 aboard.
At tick 1200 both remain airborne. Thus the empty hangar in that run was
caused by AI boarding, independently of the stale executable. No spawn or
boarding rules were changed in response to the screenshot.


## Gunship regression follow-up

The maintainer reported that the uniform fighter layout broke LAAT/i and MAF
translation. The checkout snapshot read for that report was BF1 Geonosis,
seed 42, exit tick 173, build `f4936755a270+dirty` at 20:00 local time; it
contains the player flying the Geonosian Starfighter, so it is not a recording
of the gunship failure. The loaded-class motion tests reproduce the faulty
assignment without needing that missing gunship capture.

The corrected test now drives the production `WalkSim::map_flight_sticks`
selector, rather than calling a uniform mapper directly. Each reported BF1
class has a physical-motion assertion, not only an input-channel assertion.
The maintainer subsequently confirmed the final controls work as intended.
Original-game controller comparison remains outstanding. No AI boarding rule
was changed.


The corrected release was packed from the existing owned bakes and missions with
`game --write-pack`: 28,902 entries, 1,966,790,642 bytes. Its executable prefix
matches the freshly built release (SHA-256
`2ee8aa95024bf61a7f727cff8b7309c480695171e7767fad09ed651df0e53dfd`).
From `dist/`, without loose bakes, the packed candidate passed
`--static-digest --assets assets_baked --missions missions --map bf1_geo1
--mode con --teams rep,cis`: 10 pads / 6 vehicle classes, static digest
`5f8d6efe41bb2d31`. It was then installed as `dist/openbf.exe` using the
pack script's atomic installer; all three existing non-playtest user files
retained their SHA-256 hashes. Existing assets were packed, not rebaked.


## Accepted controls and latest empty-hangar report

The maintainer confirmed the corrected controls work in the refreshed packed
build and authorized committing them. Their reported launch sequence was
Single Player -> Instant Action -> BF1 Geonosis: Spire -> Conquest ->
Separatists -> Super Battle Droid.

The latest `dist/openbf_data/playtest/last.json` was read first: seed 442,
exit tick 401 / 13.37 s, build `f4936755a270+dirty`, build time 1794698100.
It again contains both Starfighters: vehicles 10 and 12, full 4,200 HP at
tick 60, already in takeoff with bots 32 and 33 aboard. At tick 360 / 12 s
both are flying, at (31.39, -39.93, 83.61) and (28.17, -38.98, 70.66).
Thus the current executable spawns both craft; AI has taken them before the
player reaches the hangar. Whether this boarding timing matches original BF1
or makes the craft insufficiently available to a human remains unmeasured.
No AI behavior change is included in this accepted controls commit.
