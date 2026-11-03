---
id: DEKU
title: The droideka unit in the sim: its 14-state form machine (fold 0.50 s, unfold 1.67 s), the ball (13 / 12 m/s top, the PS2's doubled step), the walk form, its collision bodies, the repeater - spawned by tests, not yet fielded
status: archived
issue: 85
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #85](https://github.com/sprucey149/openbf/issues/85).

## Why
DEKA's RE (`analysis/re-droideka-unit.md` [O], `analysis/re-droideka.md`): the PS2 fields 2-4 droidekas in every BF1 Clone Wars battle and 1-2 in BF2's; the port has no such unit. This card is the unit itself in `core`, driven by a Controllable-style input (so a player or a bot can drive it later), with nothing fielded yet - every gate PASS, nothing declared.

## Evidence
- Form machine: the 14 states and the 14 x 8 transition table (ELF 0x517588 / 0x5fccc8, byte-identical; re-droideka 2), the clip times of section 4 (BF1 (frames - 1) / 30 / 2.0; BF2 RollTime / UnrollTime and the per-state rates), no fire in states 0xb..0xd.
- Ball: re-droideka 2's integrator (omega, BallAcceleration, friction, clamp) with section 7.1's doubled step: ground speed 2 x omega x r / k (BF1 13.0 m/s, BF2 12.0 m/s top), fall 39.2 m/s^2; BF2's ball jump on every droideka (PLAN Movement & feel: apex 1.27 m, 30 energy).
- Walk form (sections 5.4, 6): body yaw at once, legs catch up by 30 degree turns past TurnThreshold; BF1 2.26 m/s forward / back and no strafe, BF2 2.5 m/s with strafe; no acceleration.
- Collision (7.2): BF1 p_rollsphere r 0.65 / p_base r 1.75 h 2.0 / BldgSphere r 1.25; BF2 the MASK'd prims and coll_ball; the walk form's push-out rule; water (7.3); death (7.4: one tick, explosion, no ragdoll).
- Class params from the baked class (BASE walkerdroid; `engine/assets_baked/classes/bf{1,2}_cis.ron` already hold every prop).

## Acceptance
Tests in `core` (each printed in the report): (1) the transition table equals the ELF's 112 words; (2) fold 0.50 s and unfold 1.67 s +- one tick for both games' classes; (3) ball top speed on flat ground 13.0 (BF1) / 12.0 (BF2) m/s +- 2 %, and a drop of 5 m takes 0.505 s +- one tick (39.2 m/s^2); (4) walk-loop speed 2.26 (BF1) / 2.5 (BF2) m/s +- 3 %, BF1 strafe input turns instead; (5) the BF2-style jump reaches 1.27 m +- 0.05; (6) no shot leaves in states 0xb..0xd; (7) water over 1.5 m deals 10 per tick after 1 s. Gates: every gate PASS, nothing declared (the unit is not fielded).

## Notes
- New constants not in section 0's [O] set are PROVISIONAL (crash damage, the legs' lag).
- The shield is DEKS, the draw DEKD, the roster and the bots DEKF.
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## Planner note
Planner 2026-10-21 20:50 (the maintainer via INBOX 17:50 item 3): `first: 6` - the droideka playable and fielded first; order DEKU, DEKD, DEKF, then DEKS.

## Build notes (2026-10-21 22:15, lane 2)
- Built: `core::droideka` (branch deku b04f2b0), report `reports/20261021_2215_card_DEKU.md`. `sim.rs` not touched: nothing spawns the unit yet.
- Bar (5) as written (1.27 +- 0.05) is the continuous v0^2 / 2g. The PS2's own order (position, then gravity, twice a tick; B2:109626-109653) gives a discrete apex of 1.354 m at 30 Hz, and the test holds that value. Bar (3)'s drop takes 16 ticks = 0.533 s, inside 0.505 +- one tick.
- For DEKF / DEKD: `StepReport::weapons_off / weapons_on` must set each weapon's `active` (the PS2 stops a salvo already in progress: B1 FUN_001d0a68 slot +0x58). BF2's walk-form ordnance bodies (13 `p_-o-` bone prims) are not in `prims()`. The BF2 ball boost (sprint) is not built.
