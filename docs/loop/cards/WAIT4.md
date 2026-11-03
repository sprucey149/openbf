---
id: WAIT4
title: A new AI ground driver gives empty seats their initial four-second pickup window
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
A newly boarded bot drives away before another occupant can approach. Add the
initial four-second pickup window already specified by GUNWT, independently
of the later call/answer, claim-lifetime and player-facing extensions.

## Evidence
Existing `analysis/re-driver-wait.md` sections 1-2, 5 and 7 specify the initial
window for a ground vehicle's new driver and its early completion on full
occupancy. BF1 measures its window from taking the seat; BF2 starts its window
with the waiting agent. Use the documented clock boundaries, with one think
of observable scheduling allowance. This card inherits that existing research;
it makes no new primary-evidence or live-console claim.

On main a461f83, temporary inline caller probes naturally board at tick 1.
At tick 61, both BF1-rule and BF2-rule skiff fixtures have moved 12.802199 m,
and both AT-ST fixtures 7.410415 m; maximum locomotion stick is 1.0 in all
four empty-second-seat cases. All four expected-wait assertions fail. The
same callers with the second seat occupied by a human pass their departure
controls. Fixtures use real Sim::step/boarding, existing inline hover/walker
parameters and a distant enemy post, with no injected AI output or clock.
The AT-ST under BF1 rules is a port-only control, not a shipped BF1-class census.
`ride::gunner_coming` only holds an already claimed gunner seat; it cannot
supply an initial window when nobody has claimed it.

## Acceptance
1. Real Sim::step natural-boarding fixtures for hover and walker, both source
   rules: with an empty second seat and no claim, zero locomotion sticks and
   under 0.1 m horizontal travel during the first two seconds, then departure
   after the documented four-second window (+- one think). Log successful
   boarding, wait start/completion, sticks, position and departure ticks.
   Reproduce all four parent assertion failures with four passing full-seat
   controls; no compile/setup/zero-test result counts.
2. A full vehicle departs on the next applicable driver update; filling its
   last seat during the window ends the wait early. A vehicle whose window
   completed never restarts it on a later seat vacancy. Natural boarding and
   direct successful public entry, failed entry, exit/reboard, vehicle handle
   reuse, pilot handover and seat-switch paths have discriminating controls.
   Cover an actual walker and hover dispatch, not only a pure timer helper.
3. Human pilots and turret gunners retain their input/behaviour. Low-health
   bailout preempts the wait. A stationary wait neither exits/marks a healthy
   vehicle stuck nor spends route progress allowance. Existing gunner-claim
   holds keep their old behaviour after this new initial window. Assert the
   event/RNG controls so this slice adds no call/reply or random draw.
4. Independently remove the wait, restart it every tick, ignore full occupancy,
   retain an expired/other-vehicle clock, omit lifecycle write-back and omit
   its hash serialization: each relevant test must fail an assertion with
   restored code passing. Use reachable identical-geometry/different-clock
   continuations for the hash case. Existing default sim/trace pins stay exact.
5. Run focused ai::ride/ai::walker_drive/ai::hover_drive, complete core and game
   libraries, fmt, workspace all-target Clippy and available map/protection
   checks. Full CI and unchanged applicable sim/sweep/arena/leak/behaviour/
   bake/GPU gates remain required, with only declared sim/sweep movement.
   Restore real BF1/BF2 hover/walker maps/missions and verify empty/full pickup
   scenes and per-map boarding/stall/finish tables over 32 maps x 8 seeds;
   classify refusals by the existing NOISE/EXPOSED/CAUSED rules.
6. This slice does not satisfy GUNWT's original primary verification, UTALK
   TAT1/NAB1 rate bars, live-claim/order behaviour or GUNWH's player-hold scenes.
   Preserve those cards' original Acceptance and ordering. Missing checks
   remain unpassed; do not mark this card done on fixture passes alone.
