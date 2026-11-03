---
id: TESTMOD
title: Restore deterministic-math scanning by conforming the headless replay test module name
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
The production-source deterministic-math referee must run successfully while the
headless replay feed regression remains discoverable. Current main refuses its
test-module spelling before it can finish scanning the production file.

## Evidence
On main 37ea16d, `cargo test -p core --lib
fmath::tests::no_std_transcendentals_in_game_sim_inputs` exits 101, panicking at
core/src/fmath.rs:417: `main/headless.rs: the last #[cfg(test)] is not the
trailing mod tests`. game/src/main/headless.rs:956-957 declares the trailing
`#[cfg(test)] mod feed_tests`; the unchanged scanner accepts `mod tests`.
The module contains the existing
`replay_feed_without_hud_drains_each_tick_and_preserves_hud_delivery` test.
A source search of engine/ and scripts/ finds no code reference to feed_tests
other than that declaration. See reports/20261025_0540_card_RIDERSTATE.md.

## Acceptance
1. Reproduce the exact referee failure on the unmodified parent, then require
   exit 0 after the module rename, with 1 executed test. Preserve both logs.
2. From engine/, run `cargo test -p game --bin game
   replay_feed_without_hud_drains_each_tick_and_preserves_hud_delivery` and
   require 1 executed, passing test (zero matches is not success). Its HUD
   off/on delivery, bounded queues and sim hash assertions must all remain.
3. Run `cargo test -p core --lib`, `cargo fmt --all -- --check` and `./ci.sh`
   under the shared CI lock, sequentially; require exit 0. Classify any unrelated
   failure with its exact output rather than altering another gate or test.
4. Review the source diff: only `mod feed_tests` -> `mod tests`; no test deletion
   or production behavior change. Normal required landing gates remain intact,
   nothing declared. Missing content/GPU gates remain unpassed and preclude an
   earned landing; publish a pending-validation source checkpoint if necessary.
