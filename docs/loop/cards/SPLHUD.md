---
id: SPLHUD
title: Extract HUD-tree regressions into a child module while preserving runtime bytes and test identities
status: archived
issue: 8
---

Historical technical record. Recorded state before archival was `parked`. Current scope and ownership are on [issue #8](https://github.com/sprucey149/openbf/issues/8).

## Why / evidence
SPL15 owns the HUD tree's 1,826-line size debt. Its final `tests` module is
an existing coherent boundary: seven regressions cover the fader, message
admission/scrolling/wrapping, number colour, bar flashes and group tint.
Moving that module alone produces a 1,475-line parent and 350-line child.
The shared `fixture::target_hud` stays beside its existing users; all public
types, interpreter methods and runtime bytes stay in the parent.

On 2fa0c017a53ebc4f67be4badb66fd808c86487e9 the dry generation passes
canonical PURE MOVE: 1,431/1,431 meaningful lines, zero visibility changes,
zero new-only lines. Formatted regeneration is byte-identical; changing a
moved `hold: 2.5` to `2.6` fails the proof. Parent and candidate run the same
55 UI tests, all passing (seven under `hud_tree::tests`); candidate UI
all-target Clippy passes with warnings denied. This is a planning trial.

## Scope
Follow `engine/artifacts/briefs/SPLIT_RECIPE.md`, one file and one source
commit. Extract the existing trailing tests to `hud_tree/tests.rs`, retaining
`#[cfg(test)] mod tests;` and every test identity. Add concise module-map
documentation and update CRATES. No test addition/removal, assertion change,
new interface, caller edit, interpreter refactor or behaviour change.
Leave BHUD3/OVHT/VSEAC responsibilities and every SPL15 gate intact.

## Acceptance
1. Parent and child each at most 1,500 lines with the original responsibilities
   intact. Canonical exact-parent PURE MOVE after formatting; review all glue
   and visibility changes. Generator reproduces candidate bytes. A changed
   moved assertion/literal must fail the proof. Runtime and shared fixture
   bytes, public API and test paths remain unchanged.
2. Parent/candidate UI test lists identical, all 55 existing tests pass, no
   data skips credited. Run full UI and game libraries, workspace build,
   formatting and workspace all-target Clippy. Compare UI rustdoc warnings
   against exact parent and reject new broken links. Keep all hash/golden pins.
3. CRATES rows correct, size/map/strict checks pass after their inputs exist.
   Original SPL15/SPLIT_RECIPE full CI, applicable GPU/golden/parity/behaviour
   checks pass with unchanged HUD/spawn/map captures. No declaration,
   approval, threshold or golden changes. A source trial cannot earn this bar.
