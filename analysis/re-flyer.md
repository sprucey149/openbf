# Flyers: PS2 motion evidence and the first headless implementation

2026-11-01. This extends VFLY and corrects two claims in `re-vehicles.md`
§4.2. It is a scoped static transcription, not completed PS2 flight parity.
The existing VREST parked-height evidence remains separate.

## Evidence and corrections

Both original executable/corpus pairs were read for this work. BF1's update
is `FUN_001d9180` (B1:85041–86725); BF2's is `FUN_00202d08`
(B2:116468–119660). The class constructors are `FUN_001dead8` and
`FUN_0020f098`; setters are `FUN_001df3f0` and `FUN_0020fd58`.

The BF1 state dispatch at instruction `0x1d9dfc` loads its destination
from `0x530650`: states 0..4 go to `0x1d9e04`, `0x1d9e7c`, `0x1d9f90`,
`0x1d9ff8`, `0x1da230`. BF2's dispatch at `0x2052e8` uses `0x61bf10`:
`0x2052f0`, `0x205328`, `0x205454`, `0x205524`, `0x205780`. The executable
instructions, including the zero delay slots after each `jr`, were checked.

Two material corrections to the old summary:

- **Both games wait for clearance before advancing takeoff**, not just BF2.
- **BF2 flying-state pitch/yaw filters are rate limits**, not BF1's
  exponential relaxation. Takeoff and landing still use relaxation.

The old unnamed BF2 cap mode is **landing-region overlap**. The named Lua
callback `AddLandingRegion` (`0x18d670`, B2:33197) calls `FUN_003713a8`,
which registers region type 2 through `FUN_00371480` (B2:369159).
`FUN_00371148` queries entities in that region, calculates their sphere
overlap, writes the float at flyer+`0x668`, and sets bit 5 at +`0x660`
(B2:369066–369098). The flight update clears +`0x668` unless that bit was
refreshed (B2:116574–116580). These are hangar/landing caps, not generic
BF2 speed caps and not a distinction between atmospheric and space physics.

**Not installed yet.** The sim implements these rules against
`Sim::landing_regions`, but only tests fill that list: `scripts/gen_missions.py`
does not read `AddLandingRegion`, and no mission loader sets the list. BF2's
mission scripts do make the call. So in a real round the landing-region speed
caps and the gun block never apply, and a BF2 flyer may land anywhere.

## State and speed laws

State 0 is landed, 1 takeoff, 2 flying, 3 landing, 4 uncontrolled falling.
State 4 is also reached by abandoning an otherwise living craft:
B1:85212–85236 counts occupied seats; B2:116914–116950 additionally checks
the pilot/human occupants and a class flag. It can return to takeoff when
the craft remains alive and has an occupant. Damage can also enter it
(B1:85384; B2:117603). BF2 later enters state 5 on a death timer. Therefore
state 4 must not be interpreted as immediate destruction.

- Landed: zero linear/angular velocity and lift fraction. A takeoff-button
  press enters takeoff. The body changes from the building collision layer
  to the moving layer. Initial height is VREST's authored/model-derived
  `LandedHeight`, not an invented hover altitude.
- Takeoff: ray downward 1024 m. Lift `f` advances by `dt/TakeoffTime` only
  when clearance is greater than `LandedHeight + TakeoffHeight`; clamp at 1
  and enter flying. A new press reverses into landing only when `f > .75`.
  Relevant lines: B1:85647–85684 and B2:117855–117881.
- Flying: `f = 1`; a press requests landing. BF2's request is restricted by
  actual landing-region coverage when landing regions exist; the scalar
  region value, rather than a simple map-is-space flag, is relevant.
- Landing: reduce `f` by `dt/LandingTime`, floor at 0. A new press cancels
  only below `.25` in BF1 or `.05` in BF2. A down ray from the body's sphere
  center supplies origin clearance; touchdown requires clearance below
  `LandedHeight` and body-up dot surface-normal strictly greater than
  `.9375`. Excluded moving/flagged surfaces restart takeoff. There is no
  positional snap in this state branch (B1:85703–85793, B2:117919–117980).
- Falling: lift decreases by `.2*dt`. Gravity increases as lift disappears:
  vertical velocity receives `-9.8*(1-f)*dt`. The original also adds tumble,
  random angular disturbances and damage-dependent death timing.

Throttle `t` is in [-1,1]. Its target speed is `Mid + t*(Max-Mid)` for
nonnegative input, and `Mid - t*(Min-Mid)` for negative input. Thus neutral
throttle cruises at Mid; full back selects Min, not reverse. Clamp the
target to the current forward speed ± `Acceleraton*dt` (the ODF's spelling
really is `Acceleraton`; the setter also accepts `Acceleration`). BF1
strafe approaches `StrafeSpeed * input` by the same acceleration bound.

During takeoff/landing define `h = min(clearance/TakeoffHeight, 1)`. The
vertical component at lift 0 is `TakeoffSpeed*(.875*h+.125)` upward, or
`LandingSpeed*(.875*h+.125)` downward. Blend from that velocity toward
the steered cruise velocity by `f`. Pitch authority is multiplied by `f`,
yaw authority by `h`; leveling gains diminish with the lift fraction.

BF2 `FUN_00202c50/88/c0` gives landing-region speeds:
Max = min(authored Max, 60); Mid = min(authored Mid, 20);
Min = min(authored Min*.2, 10). Raw words at `0x5fcf24` are
60, 20, 10, .2, 3. The last value is the deceleration multiplier in a
landing region. Boost selects authored `BoostSpeed`/`BoostAcceleration`;
the latter also brakes a craft still above normal Max. The energy class
defaults to maximum 100 and recovery 20; authored `EnergyBar` changes the
maximum. A drain that crosses zero succeeds on that tick, then disables
further drains until recovery. Recovery runs below the normal speed ceiling
unless a stunt is active; ordinary boost does not block that preceding
recovery call (B2:116984–116994). That call precedes state dispatch and
therefore also recharges a previously depleted craft while landed. The port
defers first-time energy/level-timer initialization to first takeoff solely
to preserve untouched parked hash streams. Player class energy bonuses
remain a gap.

## Rotation and per-game differences

For BF1, angular speed approaches its input target by
`omega += (target-omega)*min(Filter*dt,1)`. Pitch and yaw use
`PitchRate/PitchFilter` and `TurnRate/TurnFilter`. Automatic roll leveling
uses `-LevelFilter * right.y`. The original strips the previous visual
pitch/bank before deriving the flight axes, applies angular motion, then
reapplies the new filtered visual tilt. Bank responds to local acceleration
from the angular-velocity cross linear-velocity product. BF1 divides by
MaxSpeed; BF2 divides by max(MaxSpeed, BoostSpeed), even outside boost.
Takeoff additionally divides by TurnRate in both games (B1:85954;
B2:117113,118231,118641,118758). Original quaternion integration uses
axis-angle with half-angle `length(omega)*dt/2`.

BF2 flying pitch/yaw approach their targets by at most `Filter*dt`, using
`PitchFilterDecel` and `TurnFilterDecel` when the angular motion is slowing
or reversing (B2:118455–118511). These are not `PitchDamp`/`TurnDamp` props.
The effective rate starts at min(PitchRate, TurnRate), then approaches the
axis's authored rate over stick magnitude .7..1 (B2:117111–117145).
Boost speed reduces steering; a subclass check changes the final multiplier
from .7 to .5, which is not yet represented in the implementation.

BF2's left horizontal input controls roll: deadzone .3, rescale the remaining
.7, filter with `RollFilter`, then rotate by `RollRate*dt` (B2:116999–117020,
118255–118275). Landing regions suppress that input. `LevelFilterLanding`
(hash `be55a77e`, class+`0x87c`, default 5) governs transition leveling;
`LevelFilter` (+`0x878`, default 3) governs cruise leveling. The constructor's level timer starts at 1 (B2:114515), entering flying
clears filtered roll (B2:117874–117881), and automatic leveling at timer
>= .8 clears it again (B2:118548–118550). The remaining roll-level
timer/dodge machinery needs a separate complete transcription.

BF2 cruise velocity relaxes with `clamp(VelocityFilter*dt,0,1)` (default 8),
then is rescaled to the selected forward speed when its forward projection
is positive. BF1 directly assigns the velocity from its flight axes.
Neither uses the ground hover model or a soldier's pitch clamp.

Both updates bend pitch near the Lua height band. BF2 additionally clamps
position to [minimum-.2, maximum+.2], using player limits for a human and
AI limits otherwise (B2:119057–119066). Setting out-of-band flags and the
boundary death timer are separate from that clamp; see `re-boundary.md`.

## Input and pilot aiming

The generic BF2 player controller `FUN_002b0548` floors each nonzero move
axis to magnitude .3, then divides each movement/look pair by its length
when it exceeds 1 (B2:234831–234863). These writes go to controller
+`0x6c`/`0x70` and +`0x74`/`0x78`, the flyer's throttle/roll and yaw/pitch
fields at entity +`0x2dc`..`0x2e8`. The flyer reads those stored axes
(B2:116997–117020). Thus full simultaneous throttle and roll becoming
approximately .707 each is original controller behavior. The upstream
sensitivity/remix helper `FUN_002b4598` is not fully transcribed; using
raw axes would not resolve that gap.

Pilot aim follows the **untilted aircraft forward**, not an independently
accumulated pitch. BF1 `FUN_001dbbd8` and BF2 `FUN_00209758` remove the
visual roll/pitch, store the resulting forward vector, and cast a 512 m
aim ray from the eye (B1:86920 onward; B2:120741–120805). With the port's
rotation order this is pilot relative look `(0, tilt_pitch)`; gun aimers
still follow the target through their own limits, and gunners retain
independent seat aiming.

Both updates also suppress **all seat weapons** outside flying: BF1
clears pilot and mounted-turret fire inputs at B1:86443–86482. BF2 permits
fire only in flying with zero landing-region overlap, clearing both pilot
and mounted-turret inputs otherwise (B2:118975–119007). This applies to
gunners as well as the pilot.

## Numbers checked against the actual PS2 side files

Fresh read-only `entc`/`PROP` decoding of each game's
`DATA/_LVL_PS2/SIDE/ALL.LVL` confirmed these examples. Section routing
matters: the flyer setter offers properties to the selected mounted turret
first, then recognizes its own motion fields before base setters
(B2:124864–124873,125626). `CHUNKSECTION` is handled only by the base
setter (B2:127683); it does not end the flyer's own property handling.
The BF2 space X-wing places flight tuning after its chunk declarations.
Treating every property there as chunk-only silently loses acceleration
50 and speeds 35/70/95. Turret-claimed rates must still not overwrite the
body's rates.

- BF1 X-wing: acceleration 30; Min/Mid/Max 20/50/82; pitch/yaw rates
  1/1.25; pitch/yaw filters 2.9/1.75; takeoff height/time/speed 2.5/1/3;
  landing time/speed 2/5.
- BF1 snowspeeder: acceleration 30; speeds 20/40/72; rates .8/1.5;
  filters 2.75/2.75; takeoff 2.5/1/3; landing 2/5; landed height .5.
- BF2 snowspeeder: acceleration 30; speeds 15/35/55; rates .8/.8;
  filters and deceleration filters all 4; boost speed/acceleration 100/101;
  takeoff 2.5/1/3; landing 2/5; landed height .5.
- BF2 space X-wing: acceleration 50; speeds 35/70/95; rates 1/.8;
  filters and deceleration filters all 4; roll rate 3.15; takeoff 1.5/1/10;
  landing 1/10; boost speed/acceleration 150/80; energy max/restore/drain
  80/7.5/20. In a landing region its speeds become 7/20/60.

## Implementation and outstanding fidelity

`core::vehicle::flyer` implements the state/linear/angular laws above as a
headless kernel, with separate BF1/BF2 branches. `FlyerState` adds no hash
bytes while exactly default; parked flyers therefore preserve old ground
streams. Active state, including input-edge memory and energy, is hashed.

The implementation is **not a complete original-game match**. Still owed:
actual hull contact response/collision layers; the landing sphere's origin
offset and moving/flagged-surface rejection; fractional landing-region
coverage (the implemented gate uses origin membership); all fighter dodge/stunt
maneuvers; the complete roll-level
timer; BF1 per-player full-stick rate blending and BF2 sensitivity boosts;
subclass boost-turn multipliers; transitional bank targets;
uncontrolled tumble, shared original RNG and death timers; carrier cargo.
The world adapter's swept-center collision guard, and the slide along the surface
that keeps a craft from pinning on a hangar ceiling, are provisional. So are two
rules the review of 2026-11-01 added to it, neither from the PS2: a move that began
over the terrain ends over it where the sheet is not cut (the terrain ray only
brackets a crossing, and the sheet is two-sided, so a shallow approach used to slip
under the map and stay there), and an empty falling craft that hits an up-facing
surface comes to rest there as landed (the falling state's cruise term would
otherwise skate it along the ground at MidSpeed all round, and its contact on the
PS2 is not read). A headless flight being finite and controllable does not settle
any of those questions.

No live game or PCSX2 window was opened. The next PS2 check should record
frame-boundary pose, velocity, phase, lift, input and class identity for
parked/takeoff/neutral/full/back throttle/pitch/yaw/roll/landing, on one
BF1 ground-map craft and one BF2 space craft. Use the original scripts'
landing regions, compare inside/outside controls, and test uneven ground,
abandoning the pilot seat, interrupted transitions and hull contacts. Keep
the raw residuals and have a separate reader verify the equations; none of
that acceptance is earned by the new unit tests.
