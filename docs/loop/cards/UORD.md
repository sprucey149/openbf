---
id: UORD
title: Primaries that are not bolts fly straight in the port: missile (the torpedo launchers), shell (the IMP and CIS pilots' mortar), emitterordnance (the clone pilot's arc caster), bullet (the jet trooper's EMP launcher) - 13 of the 34 fielded BF1 classes
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
UAUD1 (analysis/unit-audit-bf1.md §3.1). The port builds every primary's ordnance as a straight bolt: core/src/weapon.rs 180-181 "Gravity
is deliberately NOT read ... bolts fly straight". The disc's ordnance BASEs
(`engine/artifacts/lead_tmp/unit-audit/census_ia.md`, "Weapons") say otherwise for 13 of the 34 fielded classes.

## Evidence
- missile: `*_weap_inf_torpedo_launcher_ord` (vanguard x3, shock trooper x2, ARC trooper, assault droid; the Wookiee
  rocketeer local).
- shell: `imp_weap_inf_mortar_launcher_ord1` (pilottie, pilotatst, pilotatat), `cis_weap_inf_mortar_launcher_ord1`.
- emitterordnance: `rep_weap_inf_arccaster_ord` (clone pilot).
- bullet: `rep_weap_inf_rocket_launcher_ord` (jet trooper).

## Acceptance
- RE ([O]) per ordnance class, both games: flight (guidance and lock time, the gravity arc, the emitter's beam or
  arc), damage and splash.
- Build in `core`; sim / sweep declared. Grenades (sticky / grenade kinds) are GREN's; vehicle weapons share these
  classes (VTUR).

## Notes
- UAUD2 (2026-09-29, analysis/unit-audit-bf2.md §3.1): BF2 has 26 of 49 fielded primaries that are not bolts: missile 12 (9 rocket
  launchers, 2 EMP launchers, the MagnaGuard's Bulldog), bullet 10 (9 shotguns, the clone commander's chaingun),
  emitterordnance 4 (3 incinerators, the dark trooper's arc caster).

## Planner note (2026-09-30, M-ALPHA)
`alpha: yes` — rockets and mortars that fly straight. Value 3 (kept). Showcase maps: BF1 NAB1, BF1 TAT1 (the port's `tat1`), BF1 YAV2, BF2 UTA1, BF2 MYG1, BF2 HOT1 (PLAN §Milestones M-ALPHA).

## Note from GREN (2026-09-30, analysis/re-grenades.md §4.1, §7.3)
The shell's flight is read and measured there: gravity = `Gravity` x -9.8 added to velocity.y before the move, 30 Hz,
position += velocity x min(life, dt); under water velocity -= (0.7 dt / (0.7 dt + 1)) x (vx, vy + 0.5, vz) (B1
`FUN_0027e478`, B2 `FUN_003202d0`); launch speed = `Velocity` lerped toward the `NextCharge` class's by the charge (B1
`FUN_002762a0`); the life timer and `ExplosionExpire` are the base's (B1 `FUN_00272e98`). The AI's aim for any ordnance
with gravity is the low-arc solver (§7.3). The mortar launchers are `grenade` WEAPONS (the throw cycle of §3) firing a
`sticky` / `shell` ordnance — card GREN builds that cycle and the sticky.

- GREN landed 2026-10-02 (d4886ba): a channel-0 slot (primary or the sidearm a player switches to with R3) is still
  built by `weapon_from_db` as a bolt weapon, so the mortar launchers (a `grenade` weapon + `sticky` ordnance on
  channel 0: BF1 pilots' primary, the smuggler's and BF2 officers' second slot) fire straight bolts. The thrown path
  exists (`thrown_weapon_from_db`, `Sim::launch` — the primary's shot loop would only need to call `launch`); it was
  kept off channel 0 because bots hold these as primaries and have no ballistic aim yet (card GRAI).

## Note (planner 2026-10-12 23:40): raised 3 -> 4, after HVYW
- Card HVYW (built on branch `hvyw`, not landed) makes rocket troops fire at vehicles and at groups of 3+ - and its
  report says a rocket at a group hits ONE man, because the port's missile is a straight bolt with no blast. So the
  blast (the ordnance's explosion: radius, damage by distance, who it pushes) is the first slice of this card, before
  the arcs of the shell and the grenade launcher. Measure with HVYW's census (`lead_tmp/hvyw/tools/hvcen.py`):
  soldiers hit per rocket fired at a group, against the PS2's ExplosionName numbers on the disc.
- `deps: HVYW` for order (both edit `core/src/weapon.rs`). Undo: `loopctl.py set UORD value=3 deps=`.

## RE result (2026-10-22, lane 2) -> phase build: missiles + shells + the lock
`analysis/re-ordnance-kinds.md` (OPUS RE [O], workflow uord-re-verify: 52 verdicts, 44 SUPPORTED, 8 PARTLY fixed, 0
refuted; live PCSX2 rows in `lead_tmp/uord/ps2/`). Split: the bolt family's per-tick unit sweep -> card UORDS (value
4); the emitter -> card UORDE (value 3). This card builds, in `core` + the class loader:
1. **The missile** (BF1 B1 FUN_0027d6b8, BF2 FUN_0031f2e8; doc §4.3): constant speed (BF2: MinSpeed -> Velocity at
   Acceleration, unless Acceleration > 9000); each tick ramp = min(1, ramp + dt), turn = ramp x TurnRate; rotate the
   direction toward `desired` by min(turn dt, |dir x desired|); waver about sin(ph1) p1 + sin(ph2) p3 by |v| x ramp x
   WaverTurn x dt, phases += 2 dt WaverRate U; no gravity. desired = the launch direction, or normalize(target point -
   pos) while the target lives (soldier: feet + (0, 1.4 / 0.6 / 0.2, 0) by stance - BF2 the class table; vehicle: the
   collision-sphere centre or the Targetable body). BF2 bot missiles at non-flyers add the AI aim error (re-soldier-ai
   17.2, no range factor).
2. **Missile and shell flight**: no predictive ray; a prev -> current sweep every tick against terrain, world, hulls,
   units, water (foliage passes); first contact = direct damage + the impact explosion at point + 0.2 n, then gone;
   life-out = ExplosionExpire (both kinds). Shell: v.y += Gravity x -9.8 x dt above water (the sticky's water drag
   under), LifeSpan default 3.0. Rebound / Friction on these classes are dead keys.
3. **The lock** (doc §5): a `launcher` weapon with missile ordnance and LockTime > 0 dwells while it holds a target:
   human = the magnet target (Target* kinds: vehicles for the torpedo / rocket launchers, BF2 bulldog soldiers; BF2's
   HUD-lock cone 25.8 deg), bot = its combat target (team + Target* / BF2 AITarget* kinds, no cone); the clock
   restarts on a new target; locked at LockTime (BF1 torpedo 1.5 s, BF2 rocket 1.0 s, BF2 EMP 0 = never); each shot
   consumes it; the missile gets the target only when locked. A cannon-class missile weapon (the wrist rocket) passes
   its held target at every shot. Bots do NOT wait for the lock (no AI fire path reads it).
Slices left as notes: the BF1 mortar's charge stage on channel 0 (tap = `_ord1` shell, hold > 0.675 s = the `_ord`
sticky; `FUN_00285108`); the lock HUD (BF1's shrinking square B1:262320-262339, BF2 `player%d.missileLock*`) -> a
HUD card when this lands. (The BF2 shotgun tracers are in - the tracer module above.)

### Acceptance (build)
- Replay tests on the PS2's own rows (copy 20-40 rows of `lead_tmp/uord/ps2/b1_m_free.json`, `b2_m_free.json`,
  `b1_homing.json`, `b2_homing.json` into a test as constants): the port's missile step predicts the next velocity to
  <= 1e-3 m/s (the PS2 law fits 1,400 pairs to 1.2e-4), the BF2 speed to 1e-3, and the homing desired direction to
  0.01 deg from the target's previous-tick point. Mutation-check: reversed rotation, no ramp, waver on the wrong axis.
- Lock tests: a torpedo launcher held on a hover tank locks at 1.5 s (BF1) / a rocket launcher at 1.0 s (BF2); a press
  at 1.4 s fires unguided; a target change restarts the clock; a shot consumes the lock; a torpedo never locks a
  soldier; a bot fires without waiting.
- Shell test: `imp_weap_inf_mortar_launcher_ord1` at 45 deg from flat ground bursts in the air at 3.0 s
  (ExplosionExpire); at 20 deg it explodes on its first ground contact; no bounce.
- Census (`lead_tmp/hvyw/tools/hvcen.py` or its twin): rockets fired at vehicles, share that hit a moving vehicle,
  main beside the branch (expected up); 8 x 32 sweep 256 / 256; team kills not above GRAIA's bar.

## Note (AUDIT 2026-10-22, lane 2: re-ordnance-kinds.md PARTLY, doc fixed in f0f0dfb)
- Pick the replay rows by what they exercise: of the 1,400 pairs, 442 BF1 pairs (two 40 m/s, two 80 m/s missiles in
  `b1_m_free.json`) never changed velocity - any TurnRate fits them - and the 582 BF2 free pairs never turn (desired =
  dir; all four BF2 classes there have WaverTurn 0): they test the speed law only. The turn / waver / ramp tests take
  the four REP torpedoes of `b1_m_free.json` (0xd32c10, 0xd33870, 0xd35290, 0xd327f0: 263 pairs, law 1.1e-4, null
  0.014), `b1_homing.json` (27) and `b2_homing.json` shots 0-1 (87; shot 2 is a TurnRate-0 class that
  `homing_check.py` scores with the rocket's values and misreads at 2.67 m/s). A mutation of the BF2 waver (axis,
  sign, phase order) cannot fail on these rows: either capture the BF2 EMP launcher (ord WaverRate 0.2, WaverTurn
  0.015) or label the BF2 waver PROVISIONAL. `replay_b1.py`'s default ALL line (max 0.755) groups by speed and is not
  the doc's figure. Mutation runs: `lead_tmp/audit-uord/mut/M1..M6` (sign, sin-vs-angle clamp, ramp, phase order,
  speed law, target height).
- IMP's torpedo ordnance authors no Waver key (REP / ALL 0.5 only): read the class, never a side table.
- The mortar shell at 45 deg bursts ~6.8 m up after 90 updates of v-then-position steps (not 7.7 m): use that number
  in the shell test if it asserts a height.

## What landed 2026-10-30 (branch `uord-missiles`, 70da755 through 1b1e72b, pull request #84)
A `missile` ordnance now steers, homes on a locked target and meets the world on its own per-tick chord, and
BF2's secondary tracers spawn beside the round that carries them. Line numbers are the working tree's. This is
code, not acceptance: the card's replay criterion is still open (below).

- **The class** (`engine/crates/core/src/missile.rs:28-111`): `MissileClass` reads `TurnRate`, `WaverRate`,
  `WaverTurn`, the unnamed ramp period `0x73e5a988` (stored as `1 / value`) and BF2's `MinSpeed` /
  `Acceleration`, with the PS2's MinSpeed -> Acceleration fall-through; `speed_ramps()` (l.108-110) is BF2-only
  and off above 9000. The loader fills it for every `OrdKind::Missile` class and sets `ramps = db.is_bf2()`
  (`engine/crates/game/src/roster.rs:781-790`); the kind comes from the engine base `missile`
  (`engine/crates/core/src/weapon/ordnance.rs:118-135`).
- **The law** (`missile.rs:170-255`): ramp `min(1, r + dt)`; turn `r x TurnRate`, about `dir x desired`, clamped
  by the SINE of the angle left (Rodrigues); the two-axis waver about the launch line, its phases advanced by two
  fresh LCG draws a tick; BF2's speed ramp from `MinSpeed` to `Velocity`; no gravity. `Missile::launch`
  (l.170-191) draws the two phases and starts at `MinSpeed` / `Velocity`. The round's steering state rides on the
  bolt (`engine/crates/core/src/projectile.rs:86-88`, replay-hashed at l.190-219): `as_missile` clears the spawn
  ray's hit and the fall and attaches the steering state (l.432-446); `guide` steers one update (l.451-466).
- **The sim** (`engine/crates/core/src/sim/missiles.rs`): `probe_flight` (l.20-53) is the per-tick chord against
  terrain, water and the map's bodies (no predictive ray); `missile_aim` (l.60-65) resolves the target point (a
  soldier's `aim_point`, a vehicle's aim-sphere centre); `fly_missiles` (l.70-93) steers, then probes, and drops
  a dead handle; `tick_locks` (l.101-139) counts a launcher's dwell (human: the magnet target; bot: `ai.target`,
  no cone). Wired at step 2a (`engine/crates/core/src/sim/step.rs:499-501`) and step 5 (l.980).
- **The press** (`engine/crates/core/src/sim/ordnance.rs:141`; lock read l.238-255, missile l.272-281):
  `WeaponState::locked` gates the target the round carries at the press and every shot consumes the lock. A
  seat's missile (the cannon wrist rockets) passes its held target with no dwell
  (`engine/crates/core/src/sim/guns.rs:433-447`). `WeaponClass::lock_time` (`weapon.rs:181-187`) is read only for
  a `launcher` base (`roster.rs:886-891`); `WeaponState::{lock_target, lock_held, locked}` at
  `weapon.rs:1190-1238`, hashed at `weapon.rs:1643-1657`.
- **The draw**: a steering missile keeps one identity (its launch line and waver phases,
  `engine/crates/game/src/bolt_vis.rs:72-96`) and draws as its class's `ordnance.geometry` model
  (`bolt_vis.rs:104-111`, `missiles_in_flight` l.119-135, chained into `weapon_draws` at
  `engine/crates/game/src/soldier_vis/runtime.rs:863-866`); the trail matches it on that key alone
  (`engine/crates/game/src/fx/trail.rs:31-69`).
- **The BF2 secondary tracers** (`engine/crates/core/src/sim/ordnance/tracers.rs:29-79`; the class data at
  `weapon.rs:142-173`): a weapon that names `SecondaryOrdnanceName` / `SecondaryOrdnancePeriod` spawns a
  zero-damage `Bolt` of the tracer class beside the primary on every period-th round (`sim/ordnance.rs:305-309`),
  on the primary's ray with no extra LCG draw; a class that names a real second round (the sonic blaster) spawns
  nothing (`weapon/ordnance.rs:158-162`).
- **Tests, run 2026-10-30** from `engine/`: `cargo test -p core --lib missile` -> 14 passed, 0 failed (7 in
  `missile.rs`, 6 scene tests in `sim/missiles/tests.rs`, 1 `ai::gunner_foot` missile-line test; 1,392
  filtered). `cargo test -p core --lib a_shell_falls` -> 1 passed. `cargo test -p core --lib tracer` -> 5 passed,
  0 failed (1,406 filtered): a period-1 class rides a tracer on every round, period 5 on the fifth only, the
  tracer does no damage and dies at life-end, a class with no secondary spawns nothing, and a real second round
  (the sonic blaster) is not spawned as a tracer. `cargo test -p game --lib missile` -> 4
  passed (the two draw tests, the trail identity test, and one unrelated VO test on the filter). The scene tests
  are: homing onto a moving soldier's point and the drop of a target that dies; an unguided round's straight
  line and its `ExplosionExpire` burst (blast damage matched to `blast_falloff`); the dwell lock reaching
  LockTime and a new target restarting it; the press consuming the lock (guided at 1.0 s, unguided at 0.5 s); a
  bot locking its combat target with no magnet and no cone; a vehicle target homed on its aim-sphere centre, not
  its origin (`sim/missiles/tests.rs:139-493`).

## Every non-hero weapon draws and strikes (2026-10-30, after the maintainer's report)
- Secondaries: every `SecondaryOrdnanceName` class is registered and spawned on rounds 1, 1 + period, ... (PS2
  `FUN_0032f208`), with its own kind and the primary's charge: the sonic blaster's `fatray` blast (trail
  `imp_sfx_weap_sonicblast_ord`) and the geonosian turret's hidden bolt now fly.
- `fatray` (`sim/fatray.rs`): the PS2 sweep, piercing, falloff by distance from the line, damage floor.
- `beam` (`sim/beam.rs`): the BF2 award sniper rifles and the vehicle beams follow the fire point for their
  LifeSpan, cast Range each tick, spread their damage over the life, and draw as a ribbon (`bolt_vis::beam_draw`).
- A bullet-family round with a GeometryName draws as its model (B1 `FUN_00277130`): the BF1 jet trooper's rocket.
- RE: `analysis/re-ordnance-kinds.md` §7 and §9b.

## What is deferred (not built, with the reason)
- **Burn over time**: built 2026-10-30 from the BF2 decomp (`FUN_003156e0` keys, `FUN_00236e28` /
  `FUN_00236f50`); see card UORDE's "Burn, the shot effect and the burn bone effects".
- **Deflection, per-hit buffs, Force push / pull**: §8 names each in a clause - "deflection by the target's
  weapon", "pull (Push < 0)", "buffs" - with no key, unit or class table, and no code models them. `kick_and_push`
  returns on `push <= 0.0` (`engine/crates/core/src/sim/blast.rs:311`), so a negative `Push` (a pull) does
  nothing; the zero-damage emitter (Force push / pull) is kept on the bolt path by design
  (`sim/ordnance.rs:192-199`, test `sim/emitter.rs:431`).
- **Missile `GeometryName`**: the model path is unexercised for the Bespin map's missile round. In
  `engine/assets_baked/bf1_bes1/classes.ron:114-135` (`bes_weap_cloudcar_miss_ord`) the 15 props are the
  explosion, trail sound, damage, velocity, life and waver numbers - no `GeometryName` - and `ordnance.geometry`
  is read from that key alone (`engine/crates/core/src/weapon/ordnance.rs:270-272`), so `missiles_in_flight`
  yields no model for it and the round draws nothing. Expected, not a bug. (The brief's "the only missile
  ordnance in the bake" holds for the map file, not the class db: 33 classes in `engine/assets_baked/classes/`
  carry `base: "missile"` and the Bespin map file adds the 34th; the infantry torpedo classes do name a model -
  `rep_weap_inf_torpedo_launcher_ord` -> `com_weap_missile` at `engine/assets_baked/classes/bf1_rep.ron:2609`.) No
  in-game visual check has been made either way.

## Evidence (2026-10-30)
- Source: the files and lines above, on branch `uord-missiles` at 70da755 ("Missiles steer: the PS2 flight law,
  the launcher dwell lock and the per-tick sweep") plus the uncommitted working tree (14 modified code paths; six
  new: `core/src/emitter.rs`, `core/src/sim/emitter.rs`, `core/src/sim/missiles/`,
  `core/src/sim/ordnance/tracers.rs`, `core/src/weapon/emitter.rs`, `game/src/fx/arc.rs`).
- **The acceptance replay rows are ABSENT.** `Test-Path` is false for all four, checked from the repo root:
  `lead_tmp/uord/ps2/b1_m_free.json`, `lead_tmp/uord/ps2/b2_m_free.json`,
  `lead_tmp/uord/ps2/b1_homing.json`, `lead_tmp/uord/ps2/b2_homing.json`, and false for the same four under
  `engine/artifacts/lead_tmp/uord/ps2/`. A recursive glob finds no `*homing*.json` and no `*m_free*` path
  anywhere in the tree. The probes that wrote them are gone too: `lead_tmp/uord/ps2/missile_b1.py`,
  `replay_b1.py`, `homing_check.py` are all false. The doc's rows (`analysis/re-ordnance-kinds.md` §4.5: 1,400
  pairs, the law to 1.1e-4 m/s, 263 / 442 / 27 / 87 pairs per set) are cited there, not in the tree.
- **The replay-comparison acceptance criterion is UNSATISFIED.** No consumer exists: the criterion asks for
  20-40 of those rows as constants in a test that predicts the next velocity to <= 1e-3 m/s and the homing
  desired direction to 0.01 deg. `the_step_matches_the_documented_law` (`missile.rs:425`) compares the module
  with the doc's own pseudocode, not with the PS2 rows, and its comment says so. Restoring the four JSONs and
  adding that test (run with `cargo test -p core --lib missile`) is what closes it.
- Acceptance status, item by item:
  | item | status |
  |---|---|
  | replay rows: next velocity <= 1e-3 m/s | NOT MET - fixtures absent, no test |
  | replay rows: homing desired <= 0.01 deg | NOT MET - same |
  | mutation check (reversed rotation, no ramp, waver axis) | NOT RUN - `lead_tmp/audit-uord/mut/M1` false |
  | lock at 1.5 s (BF1) / 1.0 s (BF2) | PARTLY - the fixture class locks at 1.0 s (30 ticks); per-game class times not asserted |
  | a press before the lock fires unguided | MET (0.5 s scene) |
  | a target change restarts the clock | MET |
  | a shot consumes the lock | MET |
  | a torpedo never locks a soldier | NOT TESTED - enforced by the magnet's `target_kind` bits (`sim/input.rs:463-464`) |
  | a bot fires without waiting | BY CONSTRUCTION - the press is the only `locked()` reader (`sim/ordnance.rs:246`) |
  | shell test at 45 / 20 deg | NOT WRITTEN - the shell test in the tree is a level arc plus a 1.2 rad air burst (`sim/ordnance.rs:1534`) |
  | census 8 x 32 sweep, 256 / 256 | NOT RUN - `lead_tmp/hvyw/tools/hvcen.py` false |
- Status stays `blocked` for the missing replay fixtures and the unrun acceptance runs, not for missing code.
