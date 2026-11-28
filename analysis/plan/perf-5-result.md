# PERF-5 result: gate not met, not built (2026-11-27)

## Decision

PERF-5 (share eye-independent per-view CPU work in split frames) starts only if per-view CPU work
keeps kas2 XL above the 33.3 ms vsync step, and the expected saving would get it under that step.
The landed packets already have it under the step on the plan's p50 reading, by a clear margin.
So the gate is not met and nothing was built. PERF-0's proposed p95 reading is borderline (see
below) and does not carry the decision.

This is a bench verdict only. The night's kas2 XL 4-view frames averaged about 35 ms (two-tick
frames about 55 ms), and the bench under-reads those by about 12 ms. XL 4-view splitscreen on the
TV is still expected to run at about 30 fps after this batch; nothing here should be read as XL
reaching 40 or 60 fps. The playtest build's frame profile decides the next step (see "What would
reopen it").

Pipelined bench on littlemallard: kas2 XL, 4 views, split UI, AI seats, scale 1.0, 60 s after
1500 warm-up ticks.

| state | frame p50 / p95 ms | main CPU p50 / p95 ms | source |
|---|---|---|---|
| PERF-0b (main before PERF-2) | 31.3 / 35.2 | 30.1 / 33.9 | perf-0-result.md |
| after PERF-2 | 30.2 / 33.9 | 29.2 / 32.9 | perf-2-result.md |
| after PERF-3 (two runs) | 29.3 / 33.0; 29.4 / 33.1 | 28.5 / 32.1; 28.5 / 32.2 | perf-3-result.md |

kas2 XL pipelined has not been re-measured since PERF-4 landed (main 06ef7bdf9/4c1463873).
PERF-4 removed about 1.2 ms of GPU work from a kas2 XL 4-view frame. On PERF-0's coupling (GPU
load costs main-thread CPU time) it should lower these figures slightly, as it did on mus1 (main
CPU p50 13.5 to 12.9 ms, perf-4-result.md); that is an expectation, not a measurement.

The p50 aim (at or under 33.3) was already met at PERF-0b, 2-4 ms clear, and that is what the
decision rests on, together with PERF-0's finding that most of the night's gap is the second tick
and the unexplained 12 ms rather than per-view work. The p95 reading after PERF-3 (33.0 / 33.1)
sits 0.2-0.3 ms under the step, inside the bench's run-to-run drift (about 0.2-0.4 ms between
like-for-like runs: 33.9 in PERF-2 against 34.3 / 34.1 as PERF-3's "before"; 33.3 / 33.4 on
PERF-0's repeat). So p95 is borderline on one instrument, not met.

## Tests and measurements

None. No code changed, so the one-player gate, captures and bench runs were not needed. No new
littlemallard run was made; the figures above are the landed packets' own numbers, all from
before PERF-4. A kas2 XL 4-view pipelined run on current main would confirm the p95 margin, but
the decision does not depend on it.

## What would reopen it

- The playtest build's frame_profile (black-box) records, which exist on main without PERF-1.
  PERF-1 (partial delivery, awaiting sign-off) would add split-frame acquire and busy figures and
  session-log lines; use them if it lands, otherwise read the black-box records alone. PERF-0
  found that the bench under-reads the night by about 12 ms on kas2 two-tick frames. If the
  playtest records show kas2 XL p50 or p95 over 33.3 ms with per-view render work as the cause
  (not the second tick), the gate is met and PERF-5 runs as planned. That means call trees first (memmove callers, top `render_scene` costs, per-view draw
  counts), then a shared base static list, precomputed static world boxes and reused scratch
  Vecs.
- hot1 XL, where most frames are still over 20 ms (p50 23.4 after PERF-2). Its next vsync step
  (16.7 ms) is far off, and the frame cost is not mainly per-view CPU work. Any work there is a
  separate packet with its own target.
- The cheap, image-neutral parts (precomputed static world boxes, reused scratch Vecs) could
  later be a small CPU-tidy packet, with byte-identical captures as its proof. They are not
  built here because the gate does not ask for them.
