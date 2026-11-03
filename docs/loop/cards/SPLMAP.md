---
id: SPLMAP
title: Extract map and common bake orchestration behind the existing public entry points
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and scope
SPL7 still owns the 2,199-line map baker. Its existing `bake_content` function
is the shared map/common orchestration; the parent owns public entry points,
input reduction helpers and their existing tests. Move the complete function
without reordering any statement into private `map/content.rs`, exposing only
`pub(super) fn bake_content` and importing it privately in the parent.
The dry split is 900/1313 lines. No facade caller, schema or test moves.

## Exact seam and checks
On planning base 440ad72, move from `fn bake_content(` up to (excluding)
`#[cfg(test)] mod tests {` (original lines589..1890). Keep every other byte
except imports/module wiring and that one required visibility change. Child
imports named parent helpers/types, anyhow Context/Result/anyhow and Path;
parent no longer needs Context. Do not use a glob or widen helper visibility.
Canonical proof after fmt:1844/1844 meaningful lines, one visibility change,
zero meaningful new-only lines. Regeneration is byte-identical. Changing its
final `Ok(manifest)` to an error is rejected by proof (not a compiled mutant).
Run `cargo test -p bake --lib map::tests -- --nocapture` first, then full bake
library and unchanged lists; require synthetic map/common/nav cases to execute.
Original SPL7 scratch bake DIFF=0 and all gates remain mandatory.

## Acceptance
1. Generate from the exact worker parent with asserted, exhaustive source ranges.
   Preserve every item body, comment, field order, serde attribute/default and
   existing test body/name/order. No behavior changes, new dependencies, new
   tests, declarations, rebake, baseline edits or formatting exemptions.
2. After ordinary rustfmt, canonical `scripts/tools/pure_move_check.py`
   on the exact original file and ALL resulting files must print PURE MOVE,
   with zero lost/edited lines. Record every visibility/new-only line. Repeat
   generation plus rustfmt and require byte-identical files. A scratch edit
   to an existing meaningful line must fail proof; restore the control.
3. Compare exact parent/candidate test lists and executed results; no lost,
   renamed, added or ignored tests. Run focused and full affected library tests,
   workspace build, fmt, all-target Clippy, and compare rustdoc warnings to
   parent. Preserve all root API paths; all existing consumers must compile.
   Synthetic success never substitutes for actual bake/scene validation.
4. Both resulting files are 400..1500 lines. Update CRATES rows and remove only
   this file's size waiver. Review callers, source references and test filters.
   Map/protect and complete original parent-card/SPLIT_RECIPE gates must pass.
5. Exact-parent/candidate scratch bakes must compare DIFF=0 with actual disc
   inputs; every applicable CI, sim, motion, sweep, arena, leak, GPU, golden,
   parity and behavior gate stays unchanged. No schema or byte-format change
   is intended. A moved output is investigated, never rebaselined. Keep this
   card and its parent incomplete until those original gates pass.
