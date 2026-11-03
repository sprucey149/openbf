---
id: CAPCR
title: A player arriving on the capture tick gets assist points, not full capture credit
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
GCL16 finding 5 is a bounded event-feed bug: a player first seen in the post's
region on its completing tick can receive 5 capture points instead of the
2-point assist. The capture count should still increase by one.

## Evidence
Existing `analysis/re-event-messages.md` §9.1, lines 233–237, documents strictly
positive residence for the full-credit winner. This is an existing documented
rule, not a fresh original-game observation. `EventFeed::tick` calls
`note_capture_regions` before `capture_credit`; the latter checks for older
teammates but does not require the viewer's entry stamp to precede this tick.

On main 68a1725, temporary caller probes execute three tests: direct same-tick
arrival and a real `Sim::step` capture fail by assertion (captures, points =
(1, 5), expected (1, 2)); the prior-tick control passes with (1, 5). The real
capture occurs at tick 480 with the existing major-post timing, after an earlier
contributor charges the neutral post and leaves on the viewer's arrival tick.
No event, capture meter or feed entry stamp is injected in that real-step case.
Report: `reports/20261025_0602_planner.md`. Temporary instrumentation was removed;
unmodified event-feed tests pass 10/10 afterward.

## Scope
Split from GCL16 finding 5 only; GCL16 retains all original Acceptance and prior
validation debt. Preserve capture counts, existing positive-duration ordering
and ties, point constants, message delivery and tick deduplication. Do not
change capture eligibility, simulation, AI, timing, HUD layout or scoring tables.
Implement only the proven zero-duration boundary; do not speculate about other
original-game winner filters. Findings 4, 8 and 10 remain separate.

## Acceptance
1. Add permanent asset-free caller regressions using the existing event-feed
   scene and public `tick`/`stats`. Same-tick arrival must yield exactly one
   capture and `points::CAPTURE_ASSIST`; prior-tick residence with no earlier
   teammate retains `points::CAPTURE`. Do not seed `inside`/entry timestamps.
2. Include a real `Sim::step` capture: charge a neutral post with an earlier
   contributor through ordinary steps, replace its presence with the viewer
   on the completing tick, assert the emitted `Event::Captured`, then read
   `EventFeed::tick`/`stats`. Do not inject events or preload the capture meter.
   Preserve an older-teammate assist control and repeat-tick no-double-award.
3. Require the boundary assertion to fail on the exact unmodified parent;
   implement the smallest supported repair. Remove only its residence guard
   and require that assertion to fail again. Restore source and rerun clean.
   Positive-duration controls must remain passing. A compile error, filtered
   zero-test run or data-dependent skip is not a regression receipt.
4. Sequentially run `cargo test -p game --lib event_feed::tests -- --nocapture`,
   `cargo test -p game --lib`, `cargo test -p core --lib`,
   `cargo fmt --all -- --check`, workspace all-target Clippy and full CI;
   require exit 0 for acceptance. Keep CRATES counts current, the size cap,
   hash pins, approvals, baselines and all ordinary landing gates intact.
5. On real content, verify both boundary and ordinary capture statistics
   through the actual game/HUD and unchanged applicable CI, map/protect,
   GPU/golden/parity, sim/sweep, arena/leak and behavior gates. Retain the
   original GCL16 checks. Missing inputs permit an implementation checkpoint,
   never an earned landing or whole-card completion.
