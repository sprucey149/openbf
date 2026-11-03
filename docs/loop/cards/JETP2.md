---
id: JETP2
title: Jets, the rest - bots that jet (hop, jump-jet hints, hover attack, route arcs), the jet effect and engine sounds, BF2's fuel bar on the HUD, the jetpack_hover animation; JETP built the physics for players
status: archived
issue: 72
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #72](https://github.com/sprucey149/openbf/issues/72).

## Why
Card JETP built the jet physics on BF2's rules for every map (`analysis/re-jetpack.md` §13, the Port section):
the class data, fuel, the jump-jet, the hover, the Jedi double jump, the fly-height ceiling and the sim wiring, for
human players. A bot never jets yet (`AirEnv::human` keeps the taps a human's), nothing shows or sounds, and the
HUD has no fuel bar.

## What to build (re-jetpack.md §12 steps 9-13)
- **Verifier + captures** (§11.1-11.4): the independent verifier pass on the RE, and the PCSX2 per-update trials
  (dark trooper leap, jet trooper hover released / full forward / aim up / strafe, fuel per update) replayed against
  the port's `Soldier::step_jet`. Settle §6's backing speed (§4.2's formula gives MaxStrafeSpeed x the row's FORWARD
  factor, 7.9 m/s for the jet trooper; §6 says 6.6; the port follows the formula).
- **Bots** (§3.1 "Bots", §10): the AI mover's jet request (jump, then the jet on a later update when the lock is 0,
  the AI vector replacing the kick's direction un-normalised), the hover's pitch steering toward the wanted height
  (0.35 gain, 30 dt cap), the hop (randint(6, 12) s), the jump-jet hint (randint(3, 8) s), the hover attack (target
  20 m or more away and the fuel lasting the trip), and the roster's route flag limited to
  `SoldierConstants::has_jet` (the Jedi out). Measure the MUS1 connections 58 (26-25) and 65 (27-24) both ways with
  motion and fuel (card JETP's planner note).
- **Effect + sounds** (§8): DONE on branch `jetfx` (2026-10-28, re-jetpack.md §14): `game::roster::JetLook`,
  the engine loop + TurnOnSound / TurnOffSound (`EngineSounds::tick_jets`; TurnOffSound on every end - tap, empty,
  landing, death - since every jet class authors `TurnOffTime 0.0`, §8, open question 6 answered from code), the
  `JetEffect` on the drawn `bone_ribcage` (`fx/jet.rs`). Left: a live PS2 capture of the sound voices and of the
  effect's hard point (BF2 `DAT_005fdd48 + 0x11ab0`), the shared turn-on / turn-off handle, the Geonosians.
- **HUD** (§9): BF2's `player1jetpackbar` events (jetFuelFraction / jetFuelWarning / jetFuelThreshold /
  jetDisable) by the rule there; the bar shows for the jet trooper and not for Luke.
- **Animation** (§8): DONE on branch `jetfx`: `jetpack_hover` in both jet stances on both games (BF1 state 7 ->
  0x15, BF2 states 6 / 7 -> 0x19), weapon actions drawn over it, BF1's soft landing out of it.
- **Camera**: the jets take the sprint's section (OURS, `core::unit::camera_section`); read the PS2's own.

## Acceptance
- A bot jet trooper hops and hover-attacks on a BF2 CW map and a BF1 CW map with the scheduler intervals of §10
  over a seeded run; the MUS1 arcs traversed both ways.
- Effects and sounds on and off with the stance (built, `jetfx`; unit tests and headless stills, no windowed
  listen yet); the HUD bar for jet classes only.
- `round_to_outcome` on the jet maps: outcomes and hashes reported against the JETP baseline.
