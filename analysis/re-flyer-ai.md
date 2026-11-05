# Flyer pilots in the PS2 games

2026-11-01, VFLY / SAIR / AIRC. This is an original description of the PS2
executable and shipped data, not a decompiler excerpt. **Code-confirmed does
not mean replay-validated:** this investigation has not sampled a live PS2
flyer or compared a flight recording tick by tick.

The games need different pilot controllers. BF1 computes throttle, strafe,
yaw and pitch and lets its flyer physics move the vehicle. BF2's ordinary AI
flight advances a Catmull–Rom curve and writes the vehicle's position and
orientation directly. Feeding BF2 bots through a newly invented stick
controller would not reproduce the original's trajectory.

## Evidence and terminology

Addresses are PS2 ELF virtual addresses. `B1` means `SWBFRONT.ELF`; `B2`
means `SWBFRNT2.ELF`. The decompilation provides callers and control flow;
ELF disassembly settles the constants and property layout. Names from the
official mod-tools document
`community_mods/gametoast_lua/doc/StarWarsBattlefrontII/documentation/FlyerSplineFollowing.txt`
were matched to the PS2 property table by lowercase FNV-1a hash. That document
also names the spline algorithm, but its numeric defaults are not authoritative:
three roll defaults differ from the PS2 executable.

Relevant existing work: `re-driver-wait.md` establishes the pilot's pickup
wait and boarding claims; `re-hover-ai.md` and `re-walker-ai.md` establish the
shared BF1 steering; `re-boundary.md` covers flight-boundary recovery.

| Responsibility | BF1 address | BF2 address |
| --- | --- | --- |
| UnitFlyAgent constructor | `0x2466a8` | `0x2c80b0` |
| UnitFlyAgent update | `0x246d80` | `0x2c87c8` |
| Landing-hint search | `0x246ff0` | `0x2c8a50` |
| Upward departure probe | `0x2470e0` | `0x2c90d8` |
| Flyer entity's AI update | `0x1dd0d0` | `0x202880` |
| BF1 movement goal / speed | `0x1dcbf0` | — |
| Shared BF1 steering / aim | `0x233500` / `0x232b78` | — |
| Ordinary spline advance | — | `0x2fccd0` |
| Spline position / derivative | — | `0x539970` / `0x539b60` |
| Roll / orientation | — | `0x2feed0` / `0x2ff1e8` |
| Follower class defaults / setter | — | `0x300168` / `0x300230` |

## Boarding, departure and landing

The flyer creates a UnitFlyController when an AI takes the pilot seat. It
participates in the shared pickup wait: BF1 starts a four-second window when
the controller is created; BF2 queues a 4.001-second wait. Occupancy, live
boarding claims and BF2's human-nearby hold can shorten or extend it. See
`re-driver-wait.md` for the measured rules; an unconditional four-second
departure timer is not equivalent.

Both fly agents repeatedly check a ray **100 m upward** before departure.
When blocked they choose a forward escape state; when clear they enter the
travel state (B1 FUN_00246d80 state 3). BF1's escape builds a two-point path from the present position
to **200 m forward and 1 m up**. Landing-hint queries run no more frequently
than **every 3 s**. The query argument `0x50` has not been confirmed as a
distance and must not be described as an 80 m radius. The chosen hint is
reserved and refreshed while approaching. The agent waits for the flyer's
physical state to report landed before entering its next state.

BF1's `FlyGotoHelper` (`0x251460` through `0x2518d0`) accepts a planned
destination, an explicit path, or a direct destination. While travelling it
requests takeoff when the flyer is not already flying (state 2). The entity's
takeoff request is `0x1dd340`; its landing request is `0x1dd418`. The flyer
states observed in these readers are 0 landed, 1 taking off, 2 flying, 3
landing. BF1's fly agent can choose a command post, retrying at most ten
random candidates, with its travel point **20 m above** the post. The exact
selection and transport ownership predicates still need an independent read
before being used as a general mission planner.

## BF1 flight controls

`0x1dd0d0` skips normal steering when landed or landing. Otherwise
`0x1dcbf0` produces the current 3D goal and a speed factor. The shared
steering receives the class's MaxSpeed, StrafeSpeed, TurnRate and PitchRate.
Its throttle/strafe pair is based on horizontal goal direction and the
class's strafe capability, as documented for hovers. Flyer yaw differs in
one detail: after flattening the world goal displacement, it transforms
that displacement into the full body frame and normalizes all three local
components; the hover normalizes only the horizontal local components.

For local unit direction `l`, yaw uses `s = -l.x`; if the goal lies behind
(`l.z < 0`), replace `s` with its signed unit value. Then
`u = clamp(s * (4.5 - clamp(TurnRate, 1, 3)), -1, 1)` and yaw input is
`u * abs(u)`. Pitch uses the goal in the aim frame, with
`u = clamp(local_goal_y * (5 - clamp(PitchRate, 1, 3)), -1, 1)` and the same
signed square. These are the original's shaped controls, not tuning chosen
for the port. The pitch target retains its height.

Additional speed shaping in `0x1dcbf0` includes:

- Forward alignment multiplier `(max(dot(goal_direction, forward), 0.9) - 0.9) * 9.999998`.
- A distance blend from **0.3 to 1.0**, between **one and three MaxSpeed**
  units of goal distance. It is further blended against alignment with the
  movement order's forward vector, using an alignment threshold **0.5**.
- When this reduces the incoming speed factor, a **20 m/s / MaxSpeed**
  normalized floor. The top-level reader also applies a **5 m/s / MaxSpeed**
  floor and a floor from entity field `+0x408 / MaxSpeed`; the meaning of that
  entity field is not established here.
- Collision-goal caching and a **0.1 s** probe cadence, with a **1 s** expiry.
  The altitude/obstacle helper `0x1dc700` needs a complete follow-up read.

`0x235dc8` converts a requested speed to `clamp(speed / MaxSpeed, 0, 1)`.
`0x235e00` converts that normalized speed factor to throttle around a
class-dependent neutral point. With positive MinSpeed, neutral is **-1**;
with non-positive MinSpeed and positive MidSpeed it is
`MinSpeed / (MidSpeed - MinSpeed)`; otherwise, with positive MaxSpeed it is
`-MidSpeed / (MaxSpeed - MidSpeed)`. The positive half interpolates neutral
to +1; the negative half extends the original's other branch. The resulting
throttle multiplies the shared forward and strafe factors before the four
raw controls are written. Quantizing these controls through an i8 pad would
lose original precision, just as it does for the existing hover AI.

## BF2 spline movement

The entity AI (`0x202880`) calls `0x2fccd0` with class speed, boost ratio,
energy and the flyer's pose/velocity outputs. The follower writes the
transform; its velocity output is `(new_position - old_position) / dt`.
Normal human flyer force integration is not this path.

The four control points `P0..P3` define a uniform Catmull–Rom segment from
P1 to P2. In original mathematical notation:

```
A = -0.5*P0 + 1.5*P1 - 1.5*P2 + 0.5*P3
B = P0 - 2.5*P1 + 2*P2 - 0.5*P3
C = 0.5*(P2 - P0)
position(t) = ((A*t + B)*t + C)*t + P1
tangent(t) = (3*A*t + 2*B)*t + C
```

The ordinary update's ordering matters:

1. Update the path/pursuit/avoidance state, then evaluate position and tangent
   at the **old** parameter `t`.
2. Compute `rate = min(1.05, base_speed * speed_factor / length(tangent))`.
   A zero tangent uses rate 1 before the cap. Two maneuver flags multiply
   base speed by **0.8**; the escape timer multiplies it by **1.5**.
3. Evaluate the look-ahead tangent at
   `clamp(t + rate * PathFollowerRollLookAhead, 0, 1)`, then increment
   `t += rate * dt`. Despite the mod-tools prose calling look-ahead world
   units, the PS2 code uses it as this parameter-rate multiplier.
4. Orient the body along the current tangent. Compute roll from the change
   in look-ahead heading. Apply the separately implemented avoidance and
   docking corrections. A requested position jump over **15 m** is limited
   to **10 m**, then write position and delta/dt velocity.
5. Compute next tick's speed factor. Turn fraction is
   `clamp(abs(heading_rate / pi), 0, 1)`; interpolate from 1 to
   PathFollowerTurnSpeed with that fraction and multiply entity, path and
   node speed factors. Converge with
   `clamp(inverse_acceleration_time * dt, 0, 1)`.
6. Handle boost energy crossings (**0.2** is the threshold to begin boost),
   then advance the node/curve when `t >= 1`. Pursuit exit adds a **3.5 s**
   maneuver timer. Curve shifting and branch choice are part of this update,
   not an independent circular patrol.

Roll normally derives a desired angle from heading rate, current speed
factor and RollSpeedFactor, clamped to **±pi/2**. A trigonometric wobble is
added before filtering; its phase includes low bits of the original object's
address. Desired angle, acceleration, angular velocity and angle each carry
state. The controller brakes within a tolerance scaled by angular speed,
caps angular speed, then adds RollDampening convergence. When a path enables
PathRoll it instead interpolates the authored node roll. An exact portable
phase mapping for the address-based wobble remains unconfirmed.

The authoritative PS2 class defaults are:

| Property | Hash | Stored default / conversion |
| --- | --- | --- |
| PathFollowerSpeed | `91032dcb` | 1 |
| PathFollowerTurnSpeed | `1a2fabca` | 0.5 |
| PathFollowerAcceleration | `ab9db3f4` | 2 s, stored inverse 0.5; authored zero stores 0.0001 |
| PathFollowerRollAcceleration | `2d976c37` | pi/2 rad/s²; authored degrees converted to radians |
| PathFollowerRollLookAhead | `732ff7d1` | 1 |
| PathFollowerRollTolerance | `93425352` | pi/8 rad; authored degrees converted to radians |
| PathFollowerRollSpeed | `b1354466` | pi/2 rad/s; setter's unit conversion is not confirmed |
| PathFollowerRollAccelerationLag | `60900c05` | 0.001 s, stored inverse 1000 |
| PathFollowerRollTurnFactor | `acf747f3` | 5 |
| PathFollowerRollSpeedFactor | `e217bc71` | 1 |
| PathFollowerRollDampening | `4453dc0a` | 0.5 |
| PathFollowerBranchPaths | `9f528aec` | 0 |
| PathFollowerBranchRange | `9eececc5` | maximum finite float |
| PathFollowerBranchMaxAngle | `886f9fe5` | cos(180°) = -1; setter takes degrees |
| PathFollowerMinBranchDist | `e103ce94` | 300 m |
| PathFollowerFlyerVsFlyer | `c5cf2a8d` | true |
| PathFollowerClass | `ff158b18` | zero (discovery bypass; joining still checks limits) |

The PS2 setter at `0x300230` has explicit degree conversions for acceleration
and tolerance, but not the RollSpeed hash. Do not silently apply the PC
documentation's claimed degrees conversion to that third property.

## Authored routes and combat

The official path format requires names beginning `entitypath `. Its path
Class/Class1..4 values constrain eligible vehicle classes; Class can include
a concurrent-user limit. MaxEntities, Direction, SingleDirection, Frequency,
Speed, BranchDifferent and EnableObject further affect selection. Node
properties include Speed, BranchProbability, BranchPaths, Range, MaxAngle,
BranchRegion, VarianceX/Y/Z and LandOnArrival. Branch regions are named
`entitypathbranch NAME`. These names and meanings are document-confirmed;
the complete PS2 branch scoring and occupancy implementation still needs
transcription. Do not replace it with nearest-point selection and describe
that as confirmed PS2 behavior.

A further read locates that work precisely: `0x2fc980` decides whether to
branch, `0x2fc550` resolves class/node overrides, and `0x2ff478` selects the
endpoint. The selector checks compatible classes and concurrent-user limits
(`0x2f9740`), enabled state, forced/single direction and exclusion of the
current path/endpoint. Both ends can be candidates. `0x2f9498` scores the
distance and alignment of the vehicle-to-endpoint direction with both the
vehicle forward and the candidate's first segment. Each angle contributes
`(dot + 1) * 0.5 + 0.0001` after the maximum-angle gate. The selector
multiplies by **0.4** inside MinBranchDist, then by path Frequency and clamps
the score to [0, 1]. It ranks candidates and draws among at most BranchPaths
of the highest scores (zero chooses the first); collision rejection can
retry up to **four** choices. A connection uses the existing position minus
forward times **20 m**, the current position, the candidate endpoint and its
next node as a Catmull–Rom segment. `0x2fff48` checks three straight chords
at parameter thirds for an ordinary candidate, or two chords for a fallback.
The precise Range field/squared-distance convention and the fallback order
still need resolution before this becomes a faithful implementation.

Pursuit is not merely pointing the nose at an enemy. `0x2fd930` builds a new
curve using the present flight, target pose/velocity and a randomized
**30 m** lateral/up offset, then invokes an obstacle-aware query. The target
and replanning rules have not yet been fully transcribed. The original
`PathFollowerFlyerVsFlyer` flag gates combat departures from authored paths.
The special fly-agent helper `0x2c8b38` is consistent with the documented
CircleATATFrequency behavior: a **0.25** default, **4.75 s** polling,
**200 m** search, followed by the circle builder `0x2fe150`. It must not be
mistaken for ordinary fighter attack selection.

## What the port builds (2026-11-01)

`core::ai::flyer_drive` and `flyer::spline_step`. Its module header is the list
of invented policy; this section says what the evidence supports.

Bots fly by default since 2026-11-01 (`OPENBF_FLYER_BOTS=0` keeps them on the ground
for a run measuring the ground war alone). The PS2 has no such switch, and its code has
AI pilots (FUN_001de160), but no live sample here says how often they board on each
map. It moves the hashes of the maps with flyers a bot can fly: at 900 ticks, seed 42,
bf1_bes1, bf1_geo1, bf1_hot1, bf1_kas1, bf1_nab1, bf2_hot1 and tat1. The other 27 match.

BF1, from the code: the upward probe and escape path, the random post 20 m up
(FUN_00246718 draws any post for an ordinary pilot, whoever holds it), the
altitude law (FUN_001dc700: five terrain samples at L, 0.7L and 0.5L with
L = max(1.5 x speed, 1), the height kept 25 m over the highest, capped at
SetMaxFlyHeight, relaxing to the goal's height on arrival), the sidestep
(FUN_00235c08: max(30, 2 x MaxSpeed) along forward x up, probed each 0.1 s
over MaxSpeed m ahead, dropped when passed or a second old), the speed share,
and the stick law. FUN_00246d80 goes to state 2 on a nonzero ray result and
state 4 otherwise; reading nonzero as a hit is consistent with the path it builds
but was not traced into FUN_00161de8.

One-seat fighters have no pickup wait: `re-driver-wait.md`'s window ends at
once on a full vehicle, so only craft with a free seat hold for it.

The stick law's pitch reads the stored aim, not the transform. FUN_00232b78 takes
its pitch frame from the controllable's +0x110, which is entity +0x3b0, where
FUN_001dbbd8 stores the forward with the visual pitch and roll taken out. Yaw reads
the transform, tilt and all. The port's `flyer::untilted` is that frame.

**The port's pitch is not the PS2's law (OURS, 2026-11-01).** With that law bots
porpoised. A trace of `bf1_kas1` at 5 Hz found three causes. The law takes the goal's
height in the nose's own frame, which reads near zero or the wrong way while a goal
abeam or behind is turned to, so a raised nose went on climbing through the turn, and
at the class's 1 rad/s a turn lasts seconds. The altitude law asks for its whole rise
over a look-ahead of 1.5 s, so a slow craft or one a few metres low aimed at the ceiling. And
the port's own attack run dived at targets under the nose. So the stick's pitch error
is now the goal's elevation less the nose's, the altitude law takes at most about 20
degrees (about 45 within 8 m of the ground ahead), the stick slews at 2.5 full
deflections a second, and an attack run holds for 2 s, takes at most about 20 degrees
and breaks off by flat distance. Whether the PS2's bots porpoise is unmeasured, and these
laws may not be how it avoids it.

| 5400 ticks, seed 7, regenerated missions | saturated pitch flips a minute | mean \|vy\| | clearance | world deaths |
|---|---|---|---|---|
| `bf1_kas1`, before | 11.6 | 6.2 | 0..114 | 0 |
| `bf1_kas1`, after | 4.6 | 2.1 | 2.5..48 | 0 |
| `tat1`, before | 20.8 | 9.2 | 0..177 | 0 |
| `tat1`, after | 8.4 | 3.8 | 4..52 | 0 |
| `bf1_geo1`, before | 22.0 | 5.2 | 0..154 | 0 |
| `bf1_geo1`, after | 14.7 | 3.0 | 9..62 | 0 |

Across seeds 7, 11 and 23 after the change: `bf1_kas1` 3.8..5.2 flips a minute with up
to 2 world deaths, `tat1` 8.4..11.0 with up to 2, `bf1_bes1` up to 8 world deaths in 31
sorties (seed 11), no craft died at the map's edge. Bay exits: on `bf1_bes1` three craft
sat wedged in their bays for 180 s of a 3 minute round; a pilot now leaves after three
tries at a way out. This is a census, not a look: no one has watched them fly.

BF2, from the code: takeoff by the flight states with no AI input (the AI
update runs only in state 2, B2:117411-117417), the curve equations, rate cap,
look-ahead, 15/10 m jump limit, speed factor, roll law and FUN_002fd930's
pursuit shape; FUN_00300230's numbers are read from the class. `EntityPath`s are
real on `bf2_hot1` (six paths, named `type_EntityPath Path N` by the bake) and
the nine PS2 space levels; no other BF2 ground map's LVL holds the string, so
bots there have nothing to fly and do not claim those craft. FUN_002feed0 reads
`SetGroundFlyerMap` to halve the roll wobble from 2 rad to 1, and BF2's Hoth
missions set it.

Perception: a rider in a flyer is seen by the craft's body (§5.1's bound radius,
doubled in BF2 for a flyer), and a flyer pilot sees any vehicle's riders by that
vehicle's body. Ground bots looking at ground vehicles are unchanged.

Not confirmed or not read: the follower's base speed argument (taken as
MaxSpeed x PathFollowerSpeed); the order's forward vector in the speed share
(taken as the goal direction); whether an escape path takes the altitude lift
(the port's does not, or the hangar ceiling holds it); landing hints and their
query (`0x50`), so bots never land; attack orders in both games; BF2 branching,
Class, Frequency, Direction and node properties, which the current bake does not
keep; contact for a BF2 follower, which takes none in the port.

## Implementation boundary and remaining checks

An initial faithful implementation can cover BF1's raw stick law and BF2's
ordinary authored spline follower independently. It needs to preserve the
path metadata and node metadata from the PS2 world instead of synthesizing
patrol points. At the start of this investigation the CONFIG-path reader
retained node transforms but discarded node/path metadata; fixing that is
part of loading real flight behavior, not optional presentation work.

Dispatch mounted flyer pilots before `ai::ride::drive` falls back to the
hover path or requests exit. Reuse the existing pickup wait. Only space
missions should gain new on-foot flyer boarding decisions in this sprint:
enabling flyers in the existing ground vehicle selector would alter ground
AI choices and random consumption. New state must be hashed only when
active, so old ground-only replay state remains identical. These are port
integration constraints, not claims about the PS2's boarding policy.

Still unconfirmed: complete BF1 obstacle/height planning, both games' exact
strategic target selection, BF2 branch scoring and path occupancy, full
pursuit/avoidance/docking/landing, squadrons, and a portable reproduction of
address-dependent roll phase. Each incomplete behavior must be named when
implemented provisionally. Synthetic headless tests establish numerical
and determinism properties; they cannot prove PS2 combat or trajectory
equivalence. The remaining confirmation should record real PS2 flight
pose, path handle/node, curve parameter, speed factor and roll state each
tick, then replay these readers against that trace. Human play should check
departure clearance, banked turns, chasing and firing, return to authored
paths, and landings in a populated space battle.

Reproducible ELF checks (read-only):

```
python3 scripts/re_check.py callers b1 FUN_001dd0d0
python3 scripts/re_check.py callers b2 FUN_002fccd0
python3 scripts/re_check.py word b2 0x625de8 45 --u32
python3 scripts/tools/eedis.py bf2 300168 300230
python3 scripts/tools/eedis.py bf2 2fcfb0 2fd090
```

The callers identify the BF1 controllable dispatch and BF2 entity AI; the
45 words are the 15 property-table triples (hash, reader type, field offset).
The constructor disassembly establishes the PS2 roll constants above, and
the final disassembly establishes the 1.05 cap and look-ahead/advance order.

## Route admission and ownership (2026-11-03, issue 103)

[O] UNAUDITED, source-confirmed; original-game restricted-route trials remain
unperformed. This extends the endpoint selector evidence above. BF1's ordinary
fly agent remains stick-driven (`FUN_00246d80:162403-162544`,
`FUN_001dd0d0:87931-88024`); these BF2
EntityPath gates must not be imposed on its post destinations.

BF2 `FUN_002f90c0:286874-286914` reads the config path and dispatches path
properties to `FUN_002f9980`. The generic scope reader
`FUN_0055d5d0:694225-694237` enters `Properties` (hash `11de6cdc`), separately
from node scopes. `FUN_002f8c78:286660-286684` establishes unlimited total
capacity, no class restrictions, either direction, frequency one, enabled,
and SingleDirection off. `FUN_002f9980:287244-287400` stores MaxEntities
(`95ea8376`), Direction (`df6dc76a`, truncated and clamped to -1..1),
SingleDirection (`3311aa8e`), Frequency (`2fb31c01`), EnableObject (`c87abc4d`)
and the Name override (`8d39bde6`). Class/Class1..4 strings append up to five
class IDs; a space separates an optional decimal integer-prefix simultaneous-user
limit, otherwise unlimited. The IDs are lowercase FNV hashes, not vehicle ODF names.
`FUN_00300230:292051-292065` stores PathFollowerClass at class +0x54 and
PathFollowerClassOverride at +0x58; ordinary admission uses the former.

`FUN_002f9740:287111-287136` rejects a full total, then a wrong or full class.
An empty route class list or zero caller class permits the class test. Admission
and actual joining are distinct: `FUN_002f9808:287161-287189` increments the
total, increments the matching class only on success, and rolls total back on
failure. Consequently a zero caller class passes discovery but cannot join a
class-restricted path without a matching zero class entry. Retain that distinction.
`FUN_002f98d8:287203-287232` releases total/class and clears the remembered
direction only when the total becomes zero. `FUN_002fa4e0:287849-287859`
detaches the follower; `FUN_002fa570:287879-287909` detaches the old path
before attempting its new reservation. Pursuit and follower destruction must
therefore release ownership as well as completing a route.

`FUN_002ff478:291536-291581` performs discovery before scoring. Its call to
`FUN_002f9740` is conditional on a nonzero follower class (:291536-291540),
so class-zero discovery skips total capacity as well as class limits. Actual
joining still enforces both. openbf checks join eligibility before ranking a
final connection, then rechecks during unit application to arbitrate races.
Disabled paths
are omitted. A positive Direction permits only the first endpoint; negative
permits only the last. With Direction zero and SingleDirection on, the remembered
user direction supplies the same gate. `FUN_002f9da8:287413-287429` checks the
explicit enabled bit and, when resolvable, the EnableObject's live bit. An absent
object does not disable the path. `FUN_002f9f18:287487-287494` toggles all paths
with a matching effective name. These are runtime toggles, not an authored
Enabled path property.

Frequency multiplies the normal suitability before the [0,1] clamp
(`FUN_002ff478:291635-291643`). A zero weighted score can still enter the
separate geometric fallback (`:291644-291674`); Frequency zero is not an
absolute prohibition. openbf retains its documented geometric retry fallback
and deterministic ranking policy; this change does not claim PS2 random
branch choice, node properties, branch ranges or exact fallback ordering.

The manifest now retains actual BF2 path Properties, including strings, in an
optional `flight_props` list. BF1 binary paths and BF2 paths without Properties
serialize identically. Only manifests whose inputs genuinely contain Properties
change; meshes, collision, nav and other bake products must remain identical.
Old manifests deserialize but require a rebake to recover dropped properties.


The implementation derives route users from live AI pilots and their current
vehicle cursors, with reciprocal unit/vehicle handles. It excludes the joining
craft, rechecks a new reservation in unit application order, and clears an
authored follower on unseating. A pursuit has no cursor. Dead pilots, destroyed
craft and reused generations cannot reserve capacity. Direction is recovered
from the remaining live users, so the last release unlocks SingleDirection.
Disabled bits are hashed; occupancy is already represented by the hashed
vehicles/units/cursors. Static metadata remains setup input, as with nav data.
BF1 has no authored-route cursors and retains its previous detach behavior.

The enabling-object adapter observes the destructible instances the simulation
currently represents: building turrets and powerup stations. Other world objects
have no destructible runtime state here and remain live; arbitrary building
health and space-capital destruction are not implemented by this change. A
missing name retains the PS2's enabled default. Runtime named toggles are exposed
through `Sim::enable_flyer_path`; there is currently no production caller.
Only tests exercise named toggles; this does not add a Lua interpreter.

Owned-input source checks find 448 routes across Hoth and nine space LVLs;
429 carry Class, 165 Direction, 65 EnableObject, 20 Frequency and two
SingleDirection. All 448 route names and admission-property records agree with
the corresponding owned Xbox and PC BF2 inputs. This is a metadata comparison,
not cross-source round or original-game acceptance. Hoth and Space Yavin rebakes
change only manifests; the BF1 Bespin control stays byte-identical. Restricted
route fixtures and headless seeded rounds cannot substitute for the outstanding
PCSX2 class/direction/capacity/frequency and full flight-trajectory comparisons.


### Refused connections and numeric readers (2026-11-03)

[O] UNAUDITED source inspection, not an original-game maneuver measurement.
The endpoint list in `FUN_002ff478` is populated only after admission, enabled
and direction gates (:291536-291581). Its geometric retry also uses that list,
so it does not recover a forbidden path. The empty-ranked output has a null path
(:291739-291771). The caller `FUN_002fc550:289428-289443` attempts a reservation
only for a non-null path; either failure reaches `FUN_002fa478`. That reset
releases the path and resets follower flags (`FUN_002fa478:287794-287810`,
`FUN_002fa4e0:287849-287859`). These functions do not prove a loiter, landing
or safe post-reset flight trajectory. That still needs a PCSX2 control.

OURS, measure: capture the PS2 all-routes-refused trajectory after this reset,
then replace openbf's temporary loiter and its two seconds of MidSpeed radius.
The port uses a horizontal four-segment circle near the failed connection,
keeps no route cursor/reservation and retries admission after each segment.
A simultaneous join loser enters the same fallback. Pursuit clears the loiter
center. This avoids both an uncontrolled straight flight and an idle pilot;
it does not reconstruct collision avoidance or prove clearance around buildings.

The two limit readers differ. MaxEntities calls `FUN_00490a78`
(`FUN_002f9980:287264-287267`), which converts a float to an unsigned integer,
truncates positive fractions, maps negatives to zero and saturates overflow
(`FUN_00490a78:550912-550946`). Class strings call `FUN_00520f78`
(:287393-287399), a decimal signed integer-prefix reader through
`FUN_00525720:654082-654090` and `FUN_005253a8:653972-654079`.
Thus `fighter 2.5` means two; `fighter -1` stores the low 32 bits as unlimited,
and `fighter -2` stores 4294967294. This wrap is intentional for the string
reader; applying it to negative MaxEntities was incorrect. No-digit strings
read as zero; missing class limits remain unlimited. Overflow follows the
signed 64-bit reader before storing its low 32 bits.


The follow-up raw-disc class census resolves inherited properties across the
four main faction sides and COMMON (BF1), or INGAME and TUR (BF2). BF1 has
14 flyer/carrier entity records with no follower class. BF2 has 29 such records,
22 classified: 25 vehicle-name records (18 classified) and four weapon-name
records used by guided ordnance (all classified). Hoth's snowspeeder is class
zero, while the space X-wing and TIE fighter use `all` and `imp` respectively.
The seeded Space Yavin flight fixture now exercises those real classes against
rebaked class-restricted routes through normal simulation steps and exact replay.
It stages pilots outside hangars after normal pad spawning; it does not test
hangar escape or complete Space Assault scoring. The earlier Bespin/Hoth rounds
did not exercise real class, finite-capacity or nondefault-frequency admission;
finite-capacity and nondefault-frequency decisions still have fixture coverage
only. No original-game admission/trajectory acceptance is inferred.

### The chosen route out of a hangar (2026-11-04)

[O] `FUN_002ff478` (current lines 279157-279640) has no branch that flies a candidate whose chords failed. Ranked candidates
are drawn up to four times, each dropped when `FUN_002fff48`'s three chords fail. After that it returns the best candidate
of a separate list, filled during scoring from candidates out of range or scored zero, each tested on two chords, or a null
path. The port's `.or(ranked.first())` was therefore wrong and is gone. Not ported: that geometric list. `FUN_002fff48` also
ignores a hit unless the struck object carries flag 2 or 8 (`FUN_00168af0`); the port's chord treats any hit as a block.
`trace:` those flags.

A craft with no clear route holds with no curve and retries each tick. A class no route names gets no bot pilot. Both OURS,
`measure:` a PS2 round. The transports' case is written up in `re-space-assault.md` §10.

Flight contact and pilot rays read `Layers::VEHICLE`, so a hangar field no longer refuses a route.
