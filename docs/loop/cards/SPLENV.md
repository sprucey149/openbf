---
id: SPLENV
title: Extract environment manifest records while preserving root API and serialized bytes
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and scope
SPL10 still owns the 1,955-line manifest root. Group its environment records
(weather, lights, foliage and codec, water, fog and sky) in private
`environment.rs`, explicitly re-exporting all22 existing public names from
crate root. The dry split is1278/692 lines, with no consumer edits. Existing
root tests stay in place. This is organization only, not a schema revision.

## Exact seams and checks
On planning base440ad72 move these complete blocks including docs/derives:
- `/// The Precipitation effect` up to `/// One world's authored AI data`;
- `/// A map's water surface.` up to `/// Shared-asset bake`;
- `// --- lighting, fog and sky` up to the sounds.ron section separator.
Keep boundaries exhaustive and original relative order within each block.
Move flag_on with water and Default for ManifestLighting with lighting.
Child imports serde and crate::ManifestInstance; a cfg(doc) crate::Manifest
import keeps links resolved without a production unused import. Re-export
foliage_bin itself so existing nested paths remain valid; do not export the
private environment module. Every field order/default/comment remains exact.
Canonical after fmt:1716/1717 meaningful lines, zero visibility changes;
the sole new meaningful line is cfg(doc). Regeneration byte-identical;
changing water height f32 to f64 fails proof (not a compiled mutant).
Run `cargo test -p manifest --lib -- --nocapture` first (22 existing tests),
then full bake library and workspace consumers. Compare rustdoc warnings;
all wire-order/legacy-default/foliage-codec tests must execute unchanged.
SPL10's scratch bake DIFF=0 and wall-slice proof debt remain mandatory.

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
