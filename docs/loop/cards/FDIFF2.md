---
id: FDIFF2
title: State-sync oracle: inject a measured unit state at tick k and report matching ticks under a recorded pad
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
A selected-state trace cannot restore all future-affecting state. State-sync
needs an explicit engine injection boundary before a matching-duration number
can be trusted. FDIFF handles read-only trace comparison; it never injects state.

## Do
1. Define the state and recorded input needed to synchronize one soldier's motion
   at measured tick k, including velocity, generation, grounded state and clocks.
   Identify unobserved fields explicitly from the oracle-state evidence.
2. Design a narrow injection API outside the normal tick path. Reject unsupported
   state rather than inventing values or calling the trace a complete save state.
3. Seed the port from the PS2 sample at k, replay the same recorded pad and use
   FDIFF's tolerances to report matching tick count and first differing field.

## Acceptance
- Research output lists each injected field, source evidence, unavailable fields,
  and the exact API/CLI and runnable check for the build phase.
- Build acceptance: immediate re-export after injection matches the source sample's
  selected fields exactly; two runs of the same seed/input produce the same result.
- A planted motion mutation produces a known shorter matching window and names
  the first tick/field; the null control matches for 600 ticks.
- One real PS2 soldier-motion replay at a named tick k reports the matching
  duration, tolerances, both values at divergence and all uninjected-state caveats.
  Preserve the raw sample/input and run metadata as evidence.

## Notes
Split from FDIFF as allowed by its Do/Notes: engine changes belong here.
