---
id: TESTEV
title: Keep event-audio regressions discoverable while restoring the deterministic-math source scan
milestone: M0
value: 4
phase: cleanup
status: done
deps:
files: engine/crates/game/src/event_sounds.rs, engine/crates/game/src/event_sounds/contact_tests.rs, engine/crates/game/src/event_sounds/tests.rs (new)
declares:
needs:
effort:
branch: testev
resume:
created: 2026-10-25
tries: 1
---

## Why
HITSND's new trailing test-module declaration stops the core deterministic-math
referee before it finishes scanning game production inputs. All impact-audio
regressions must remain discoverable while the existing scanner runs again.

## Evidence
On fa54172, after exact restoration of unrelated planner probes, `cargo test
-p core --lib` exits101:845passed/1failed. The only failure is
`fmath::tests::no_std_transcendentals_in_game_sim_inputs` at fmath.rs:417:
`event_sounds.rs: the last #[cfg(test)] is not the trailing mod tests`.
The last declaration is cfg(test), an explicit path attribute, then
`mod contact_tests;`. The scanner accepts its established trailing `mod tests;`
convention. The earlier inline module contains four event/tick-delivery tests;
the separate file contains twelve actual-contact caller tests.

A temporary layout diagnostic moves the separate file byte for byte to
`event_sounds/tests.rs`, renames the existing inline module to `tick_tests`,
and ends event_sounds.rs with cfg(test) + `mod tests;`, without the path
attribute. The scanner passes1/1, the whole core library846/846, and
`cargo test -p game --lib event_sounds -- --nocapture` passes16/16.
This is evidence for the repair, not a committed implementation or acceptance.

## Acceptance
1. Reproduce the parent scanner assertion with one executed test, then require
   its pass after the layout correction. Restoring only the old module layout
   must reproduce the same assertion, never a compiler/setup/zero-test result.
2. Run `cargo test -p game --lib event_sounds -- --nocapture`; require all16
   current tests pass (12 contact callers +4 original event/tick checks).
   Compare test names/bodies against the parent; only namespace changes allowed.
   Verify moved-file bytes and production prefix equality. Run the complete
   game and core libraries, fmt and workspace all-target Clippy sequentially.
3. Full `./ci.sh` and unchanged applicable cleanup/landing gates must pass with
   nothing declared. A missing content/GPU check remains unpassed; separate any
   inherited blocker with its exact receipt. Do not change pins or gate inputs.
4. Keep pending local acceptance for HITSND, VOICEP, GSND and earlier changes;
   any implementation checkpoint stays incomplete until normal gates are earned.
