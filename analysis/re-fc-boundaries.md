# Thrown-item bounds and fixed-tick boundary audit (issue #103)

2026-11-03. [O] UNAUDITED. This is source review and deterministic port
reproduction against existing measurements, not a new original-game run.
Hero, saber, Force and combo systems are excluded from this slice.

## 1. Item release strength

The PC's `WeaponDispenser::Fire` (`0x00683c20`) has no known PS2 counterpart yet.
The BF2 PS2 dispenser instance vtable's release slot at `0x628d24` points
to `FUN_00335920`. Its throw speed uses the stored strength at instance
`+0x124` to interpolate between class `+0x258` and `+0x25c`, then multiplies
by class velocity `+0x244`, before adding owner velocity
(B2:328611–328629). No final strength clamp intervenes in that expression.
BF1 `FUN_0028d958` has the same operation with strength `+0x144`, class
endpoints `+0x1f4` / `+0x1f8` and velocity `+0x1e4`
(B1:214939–214955). These are primary-source counterparts, located by the
vtable and field/behavior chain, not by a name match alone.

Charge entry resets the timer and selects MaxPressedTime: BF2
`FUN_003356e8` (B2:328083–328089), BF1 `FUN_0028d788`
(B1:214639–214643). Charge update saturates at that selected limit:
BF2 `FUN_00335718` (B2:328095–328105), BF1 `FUN_0028d7b8`
(B1:214649–214659). BF1 fire entry divides the timer by MaxPressedTime,
using strength one for a nonpositive duration (`FUN_0028d5d8`,
B1:214568–214575). BF2's fire-entry vtable slot at `0x628cd8` points to
`0x335478`; that body is missing from this decompilation, so this audit
does not claim to have verified its division from decompiler text.

The original PC game's dispenser
fire-entry, charge-entry/update and Fire records (`0x00683930`,
`0x00683ab0`, `0x00683ad0`, `0x00683c20`) agree with the visible PS2
field chain. No implementation or decompiler text is included here.

In the port, a press while the preceding fire cycle is running does not
execute the idle fresh-press reset. If held through return to idle,
`refire_accum` saturates at ShotDelay. A 2 s delay and 0.5 s charge duration
produce raw release charge **4**; endpoints 0.3/1 and velocity 10 produce
**31 m/s**, beyond the authored **10 m/s** maximum. Holstering while charging
and tiny positive durations also pass through the shared cycle; nonpositive
durations use strength one. The new tests cover those input sequences.

**FC-BOUNDS, named gameplay correction:** `Dispenser::speed` clamps release
charge to [0,1] for every source, covering both pack and mine placement.
This is a deliberate final bound beyond the original raw interpolation.
It leaves in-range interpolation, the generic WeaponState/ShotSpec charge,
weapon stages and other ordnance unchanged. It does not claim full PS2
charge-state, switching or interruption parity. Finite authored strengths
and velocity remain required; this is not general malformed-data sanitation.

Fingerprints change when an out-of-range item release changes its initial
velocity and subsequent positions/collisions. No hash schema or stored
weapon charge changes. At each endpoint the pack and mine runtime fixtures
produce identical velocities and whole-sim hashes for out-of-range input
and its clamped endpoint, under both map rules. Stationary hash pins need
no rewrite because those scenarios never throw an item.

## 2. Timer inventory and arithmetic

Audited source sites below are relative to `engine/crates/core/src`.
Locations name functions/fields rather than a line that changes with fixes.
The shared step is f32 1/30 (`TICK_DT`, bits `0x3d088889`). A comparison's
strictness, clock reset row and test-before/after-update order all matter.
A round duration alone does not establish a boundary update on the PS2.

Primary requested systems:

- `weapon.rs`, `WeaponState::update`: `reload_timer -= dt`, completes at
  `<= 0`. `re-ammo-reload.md` explicitly has no live reload timing measurement.
  At 1.5 s, repeated f32 subtraction takes 46 ticks, versus 45 nominal;
  0.3 s takes 10, versus 9 nominal. **Retain pending PS2 boundary rows.**
- `weapon.rs`, charge idle and `fire_cycle`: accumulation capped at the
  selected period; refire deficit and salvo countdown consume only the
  portion of the tick needed, carry remainder and loop. `FIRE_EPS` already
  bounds the remaining-step loop. Do not replace this with a rounded
  countdown; it supports several shots in one tick.
- `weapon/repair.rs`, `repair_tick`: saturating refire countdown, compare to
  `FIRE_EPS`, then add ShotDelay. No new measured boundary; retain.
- `sim/spawn.rs`, `SpawnWaves::tick`: add dt, compare `>= delay`, subtract
  delay, retain remainder; `cur_slot` truncates timer/interval. Native teams
  call the same wave clock. At delay 10 the arithmetic crosses on ticks
  300/600/900/1200/1500/1800; delay 15 on 450/900/1350/1800. A 0.25 s slot
  is 7.5 ticks and necessarily alternates quantized boundaries. The source
  scheme in `re-tickets-spawn.md` is not a measured per-tick wave trace;
  no wave-clock change is justified here.
- `sim/missiles.rs`, `tick_locks`: first target row sets `lock_held = 0`,
  subsequent unchanged-target rows add dt; `weapon.rs::locked` uses
  `>= LockTime`. The existing production regression pins 30 counted ticks
  for 1 s, plus the acquisition row. `re-ordnance-kinds.md` §5 gives code
  semantics, not measured lock-clock rows; retain.
- `aim_assist.rs`, `magnet`: refill on fire, test pull, then subtract dt;
  its strict positive test and before-decrement ordering differ from
  reload completion. No new boundary evidence; retain.
- `soldier/step.rs`: `time_in_stance` / `stand_age += dt`, strict `>` roll,
  crouch-rise and throw-landing gates; saturating `lock_timer - dt` then
  `<= 0` clears locks. `soldier.rs` input previews mirror that ordering.
  Prone lock durations are half-frame values (39.5/30, 44.5/30), not exact
  integral-tick clocks. Do not shorten them with a global epsilon.
- `soldier.rs::resolve_ground`: `air_time += dt`, strict `>` stand/sprint,
  crouch and prone air limits. The measured BF2 one-second stand/sprint
  boundary is corrected below. BF1's fall timing and crouch/prone air
  boundaries lack matching cadence measurements; retain their comparisons.
- `sim/droidekas.rs`: increments the shared soldier stance clock; separate
  `droideka.rs` air and water clocks remain unchanged. Hero-owned clocks
  were not audited or modified.

Other direct countdown/elapsed families found in the non-hero core scan:
`vehicle_pad.rs::count_down` (spawn); `sim/vehicles.rs` (empty/dead vehicle
lifetime); `vehicle/walker.rs` (support and gait clocks), `vehicle/hover.rs`
(jump and grounding), `vehicle/flyer.rs` (escape);
`station_motion.rs` (idle/serving state), `sim/stations.rs` (service interval);
`projectile.rs`, `sticky.rs`, `guided.rs`, `remote.rs`, `autoturret.rs`
(lifetimes/arming); `sim/mines.rs`, `sim/pickups.rs`, `sim/beam.rs`,
`sim/tow.rs`, `sim/remotes.rs`, `sim/turrets.rs` (lifetimes/think/strike);
`sim/cloaks.rs`, `sim/buffs.rs` (effect clocks); `announcer.rs` and `music.rs`
(repeat/playback clocks); `ai/hover_drive.rs` (stuck clocks). `cp.rs` adds
weighted dt for capture/neutralization, and `battle.rs` integrates fractional
ticket bleed; those are rate integrators, not constant-step timer counters.
This inventory is not an original-game acceptance claim for these systems.
No unsupported blanket rounding change was made.

## 3. Measured stance boundary and retained differences

`re-soldier-ground.md` §2.3 records BF2 stand at timer 1.000 and Fall on
the next update. Its source law is strict `> 1.0` in `FUN_00240bf8`
(B2:160284–160296, sprint B2:160506–160521). The port's repeated f32
adds reach **1.0000003576 after 30 ticks**, so a fixture starting at the
trace's airborne/reset row enters Fall on tick 30 instead of 31.
This is one tick early, rather than the countdown's possible extra tick.

**FC-FALL correction:** tolerate four f32 epsilons at the BF2 one-second
stand/sprint boundary (0.0000004768 s). This absorbs that measured arithmetic
error while remaining far below one 1/30 s step; the next tick still falls.
The stored clock, BF1 comparisons, jump windows, crouch/prone timers and
landing locks are unchanged. It is not an EE floating-point emulator.
Hashes can change when prolonged BF2 stand/sprint airtime reaches this
boundary: the Fall transition and subsequent integration occur one tick
later. Stationary pins do not encounter it.

`re-stance-roll.md` §4.3d records BF2's positive landing-lock residue at L+9
and the original pad masking/edge delay. The current port clears its 0.3 s
countdown on the tenth decrement. JUNI's existing regression accepts a
fresh jump after nine release ticks and its press tick; that port input
ordering is retained. A blanket epsilon would shorten it to nine decrements
and would erase, rather than reproduce, the documented PS2 residue. The
port's common BF2-style lock and jump windows on BF1 maps remain existing
merged-game decisions, not new PS2 parity claims from this audit.

## 4. Foley: evidence insufficient for a change

`re-foley.md` §4.1/4.3 derives steps from drawn foot movement; §8.2 records
jump and land spacing but does not say that landing and a step cannot share
an update. It also does not record the first step after draw re-entry.
Accordingly `game/src/foley.rs` is unchanged.

PCSX2 check to perform for both games: capture update-indexed stance,
grounded/drawn status, movement clip, both foot heights/previous foot
positions and started sound hashes. Compare stand-jump-land and moving
jump-land, then a continuously moving soldier leaving and returning to
view. Align the land sound and first resumed step to the same update;
control for continuous visibility, camera cuts, both starting feet and the
animator being released versus retained. Repeat on two surfaces. Only those
rows can establish whether suppressing a landing/re-entry step is correct.
