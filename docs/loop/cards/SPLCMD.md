---
id: SPLCMD
title: Extract the commander regression module without changing runtime code or test identities
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why and scope
SPL7 still owns the remaining oversized files. Its commander file has 2,500
lines: 1,079 before the trailing test module and 1,418 inside that module.
The regression module is an existing responsibility boundary. Extract it to
`commander/tests.rs`, leaving `#[cfg(test)] mod tests;` in the parent.
The projected files are 1,081 and 1,419 lines (one new module header included),
both over 400 and below 1,500. This one-module extraction needs no production
split, new abstraction, visibility widening or caller change. SPL7's bake/map
work and its complete original acceptance remain separate and unfinished.

## Acceptance
1. Use SPLIT_RECIPE for this single extraction, generated from the exact parent
   with asserted ranges/partition. Preserve every production line, test,
   fixture, assertion, test identity and ordering; only module wiring, test
   indentation and a new module header change. No new or removed tests.
2. The canonical `scripts/tools/pure_move_check.py <parent>:engine/crates/core/src/commander.rs
   engine/crates/core/src/commander.rs engine/crates/core/src/commander/tests.rs`
   prints PURE MOVE with zero lost/edited lines, zero visibility changes and
   zero meaningful new-only lines. Prove regeneration is byte-identical.
   A scratch assertion-edit mutant must fail this proof; then restore it.
3. Parent/candidate `cargo test -p core --lib -- --list` outputs are identical;
   all 37 existing commander tests still execute with the same names. Full
   core and game library tests, workspace build, fmt, all-target Clippy and
   the host-math/source scanner pass. Compare rustdoc warnings with the parent.
   Compile a deliberate TeamPlan hash-field omission, require the existing
   plan-hash test to fail, then restore and rerun the control. Do not weaken
   that referee or make a permanent test solely for this extraction.
4. Both files stay below 1,500 lines. Update both CRATES rows and remove only
   the commander waiver; inspect stale path references and named test filters.
   Map/protect checks pass when their required inputs are available. Any
   unrelated inherited failure remains reported, never waived by this card.
5. All original SPL7/SPLIT_RECIPE gates stay mandatory: full CI, GPU/goldens
   unchanged, sim hashes/motion identical, paired sweep and arenas byte-equal
   to the exact parent, no declarations, rebake or baseline/threshold edits.
   The card is not done until those gates pass. A moved value means investigate
   the extraction; it is never a reason to rebaseline.

## Planning evidence
Base c19b719. Dry-generated files in the planner kit give 2,013 meaningful
lines before and after, zero visibility changes and zero new-only lines;
PURE MOVE. Changing the existing plan-hash assert_ne to assert_eq fails the
proof with one lost/edited line. This is a proof-tool control, not a compiled
behavior mutation. Unmodified main runs 37 commander tests and 986 core tests,
all passing. The generated extraction has not yet been compiled or integrated.
