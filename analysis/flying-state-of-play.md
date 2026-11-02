# Flying and space in Instant Action — state of play

2026-11-01, source `857c8e109baa767e6328d325d33f15c2d68c7946`.

## Where this stands (sprint/flying, 2026-11-01)

Steps 1 and 2 below are built for the headless sim. A human can fly every flyer
on the ground maps with the pad, and bots fly them by default (`re-flyer-ai.md`). Steps
3 and 4 are not finished: see the section at the end. Landing regions reach the sim from the missions, and only the space
scripts make the call, so their rules apply on no ground map (`re-flyer.md`). See `re-flyer.md` and
`re-flyer-ai.md` for what is confirmed, and
the "What exists" list below for the state before this work.

## Space Assault (sprint/space, 2026-11-01): loads, flies, cannot end, so gated

Behind `OPENBF_SPACE=1`, and only after `OPENBF_BAKE_SPACE=1 scripts/rebake.sh maps` (the default rebake still skips
the six maps) and `scripts/gen_missions.py --space`, which writes a `<key>_ass.ron` for each of spa1, 3, 6, 7, 8 and 9,
running the `_cmn` script before the `_ass` one as the disc does. Keep those missions out of the shared folder for now:
several tests read every mission there and expect a ground map's roster and heroes. `mode_layer_index` picks the objective layer of each. A hangar's
`com_bldg_controlzone_ctf`, which authors no capture region, is a post that spawns its team and cannot be taken. The
scripts' `AddLandingRegion` now reach `Sim::landing_regions`, so their speed caps and gun block apply.

Headless, all six maps, 5400 ticks, seed 7: both teams spawn in their hangars, and between 20 and 45 sorties fly the
authored `EntityPath`s for 648 to 2746 airborne seconds in the round, with no panic. **Not built:**

- Capital ships. Nothing can damage a critical system, and the port has no destructible world object to hang them on, so
  `SpaceAssaultAddCriticalSystem`, `LinkedShields`, `LinkedDestroyables` and `LinkedTurrets` have no target. The
  disc's point values are in `ObjectiveSpaceAssault.lua`'s template (18 a system, engines 3 to 18).
- Scoring. The score limit is `SpaceAssaultGetScoreLimit` (`DAT_005fa898[...]`) and `AddSpaceAssaultDestroyPoints` calls
  `FUN_003616f0(.., 0x20, ..)`; neither is transcribed, nor is what a kill is worth. Until they are, a round runs to its
  time limit at best, and no one wins. That is the gate's reason.
- Bot gunnery in space: 0 to 2 shots a round from BF2's path followers, and most sorties end with the pilot leaving
  (14 to 32 falls a round). Bots do not attack ships. Marines and hangar assaults do not run.
- The HUD's space elements, the spawn screen's look on these maps, and any human flight on them: no window was opened.
- The space mission's `layers` fact reads the sky's `ReadDataFile('SPA\\spa_sky.lvl', 'myg')` second argument.
  Only the mission's comment uses it.

## What exists

- Vehicle pads recognize `flyer`, `commandflyer` and `carrier`. They build the
  seats, weapons, hull health and parked `LandedHeight`; models and rider views
  use the shared vehicle path (`game::vehicle_pads`, `core::vehicle`).
- Hover and walker movement have dedicated integrators. Flyers have neither a
  class-parameter reader nor an integrator: a spawned fighter remains parked.
  The common pad/seat inputs alone do not make it fly.
- Ground vehicle AI explicitly selects hovers and walkers. Soldier AI research
  (SAIR) and soldier air steering (AIRC) do not establish a pilot's behavior.
- The Instant Action catalog names the six PS2 space maps; see the section above
  for what opening them gives. No runtime Space Assault objective exists.

## Missing, in implementation order

1. **Flight physics and headless controls.** Read scoped BODY properties from PS2
   classes; implement landed, takeoff, flying, landing and unpowered states,
   throttle, attitude, boost and altitude restrictions. Connect ordinary pad
   controls and provide a deterministic scripted pilot example. Test state
   boundaries and real BF1/BF2 classes before committing the working step.
2. **Flying bots.** Trace original pilot acquisition, steering and attack rules;
   connect boarding, takeoff, navigation and weapon use to the same flight
   integrator. Exercise real fighters headlessly; mark any replacement policy.
3. **Space map loading and Instant Action.** Inventory both eras of each shipped
   PS2 space mission; load the correct world/layers, sides, spawn points and
   pilot classes. Make valid missions selectable and run their normal loading
   path without a window. Missing required content must produce a useful error.
4. **Capital ships and space objectives.** Load authored subsystem objects and
   shield links, target/damage them, award original mission scores and finish a
   Space Assault round. Verify external attacks and hangar/interior objectives
   through ordinary sim actions, including destruction and repeat-score guards.

## PS2 evidence and unresolved questions

`re-vehicles.md` §4.2 and `re-vehicle-rest.md` are starting references. The
authoritative update functions are BF1 `FUN_001d9180` and BF2 `FUN_00202d08`;
setters are `FUN_001df3f0` and `FUN_0020fd58`. Class numbers come from PS2 LVLs,
not PC tuning. Initial fresh static inspection confirms neutral throttle uses
MidSpeed, endpoints use Min/MaxSpeed, and velocity approaches its target by
Acceleraton × dt. Both games gate takeoff progress on LandedHeight plus
TakeoffHeight. State 4 is unpowered/falling and can be entered by losing the
last occupant; calling it simply “crashing” was too strong.

VFLY's old note leaves dispatch details, contact response, BF2 mode caps and the
human altitude-limit gate unresolved. New evidence will identify function/line
and raw constants, and distinguish static confirmation from live measurement.
Space scoring must come from the PS2 mission scripts and Space Assault helpers;
pilot decisions require their own executable trace. Port tests prove our
implementation, not console parity. No live PS2 comparison is claimed here.

Keep unrelated ground simulation and its hash byte stream unchanged. Capture
parent hashes before edits and compare unchanged ground scenarios after each
core step; new active-flight state must not alter parked/ground-only hashes.
Explicitly record any deliberate flying-scenario movement separately.
