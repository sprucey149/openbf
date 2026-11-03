---
id: VSWP
title: Seats the way the maintainer wants them (his words 2026-10-07): a player entering a teammate's vehicle takes a FREE seat first and kicks a bot only when none is free, and the weapon-switch button swaps seats WITH the teammate bot in the target seat, so the vehicle never empties
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
The maintainer, 2026-10-07: "a user should always be able to get in a teammate bot occupied vehicle, not an enemy one though of course. Also some vehicles have multiple stations and the originals had a way of switching between the stations - I think it was just the weapon switch button. When this happens, just swap the teammate bot that was in the position you switched to with where you were so the vehicle doesn't empty by you switching stations. If a user takes a bot occupied vehicle and there isn't enough room, it just kicks out the bot."

## Evidence
- **Entering** (`Sim::enter_seat`, B1 FUN_0020b1b0): the seat is the FIRST boardable seat in ring order that is empty or held by an AI. So a player facing a hover with a bot pilot and a free gunner seat takes the PILOT seat and the bot is exited and placed (`enter_vehicle`); a free seat is not preferred. An enemy vehicle is refused (`v.team`). This is the PS2's rule as read.
- **Switching** (`switch_pass`, `next_seat`, B1 FUN_0020b5e8): the SWITCH_WEAPON edge (R1 since 2026-10-04) moves to the next boardable seat that is EMPTY; a seat held by a teammate bot is skipped, so with every other seat taken the press does nothing, and with one free seat the soldier leaves his seat vacant. The PS2 has a swap for AI: 'when the exit is a seat change (argument 2), an AI in a type-2 seat is moved into the one left' (B1:120689-120718, noted in `seats.rs`'s header as VAI's) - the port's switch uses `unseat_unit` + `seat_unit` and never takes that path.

## Acceptance
- Entering: a free boardable seat is taken before an AI-held one, in ring order among free seats; the AI is kicked (as today) only when no seat is free. An enemy vehicle is still refused.
- Switching (the maintainer's rule, 2026-10-07, restated): the press goes through the seats in seat order and skips ONLY a seat held by another human; a seat held by a bot is not skipped - the player and that bot trade seats, the bot into the seat the player just left (instantly, no exit placement), so no bot is kicked out and the vehicle never empties; an empty seat is simply taken. Tests: a 2-seat and a 3-seat hover with bots in every seat (the occupants after a press are the same units, two of them traded), with a second human in one seat (it is skipped), and with a free seat. Named deviations in FEEL_NOTES (this, and the first-seat entry above).
- ALL seats, passenger seats included (the maintainer, 2026-10-07: "have the vehicle seat change loop in order through ALL seats, including passenger seats"): the press cycles through every boardable seat in seat order and wraps from the last to the first. `next_seat` today refuses a passenger-to-passenger move (`!(passenger(from) && passenger(j))`); that refusal goes. A named deviation from the PS2's FUN_0020b5e8. The test covers a vehicle with two or more passenger seats: the press visits each and wraps.

## Notes
Filed 2026-10-07 by Claude in chat from the maintainer's questions. His words are quoted; the code facts were read by Claude from `core/src/sim/seats.rs`, `sim/guns.rs` and `ai/ride.rs` the same day.
