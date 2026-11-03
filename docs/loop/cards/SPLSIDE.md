---
id: SPLSIDE
title: Extract soldier and weapon bake output from model and animation conversion
status: archived
---

Historical technical record. Recorded state before archival was `parked`.

## Why and exact scope
SPL14 still names the 1,637-line soldier baker. Its contiguous disk-output
section mixes soldier/weapon orchestration with model conversion, pose rigs,
clip conversion and their tests. Move that complete section into private
`soldier/output.rs`; keep conversions, shared types and all tests in the parent.
This responsibility can be extracted independently of SPL14's other files.

On planning base fb631f7a9dd7a625670109fc7780f51106b2fcbf, partition the
original file exhaustively into lines 1..539, 540..942 and 943..1637.
The middle block starts at `/// Bake one soldier model (+ textures)` and
ends immediately before `#[cfg(test)] mod tests`. It includes both bake
entry points, texture flags/writing, BakedWeapon and weapon-node extraction.
Re-export exactly `bake_soldier`, `bake_soldier_anims`,
`bake_textures_flagged`, `bake_weapon`, `texture_flags`, `weapon_nodes`,
and `BakedWeapon` from the original module; every consumer path stays valid.
Keep bake_flags private in the child. Child imports parent conversion helpers
explicitly; no visibility widening, glob dependency imports or caller edits.
Root anim_sets, skinned_to_glb and Path imports become test-only; Clip moves
to the child. Preserve test bodies, names, order and all item documentation.
Dry formatted sizes: parent 1,244 lines, child 416.

## Acceptance
1. Follow SPLIT_RECIPE on the exact implementation parent. Generate from
   asserted anchors and exhaustive ranges; preserve every item body, comment,
   arithmetic order, format, default and public path. No new abstraction,
   test, dependency, declaration, rebake or formatting exemption.
2. After ordinary rustfmt, canonical `scripts/tools/pure_move_check.py`
   against the original soldier.rs and BOTH resulting files prints PURE MOVE
   with zero lost/edited lines. Planning proof is 1345/1345 meaningful lines,
   zero visibility and new-only lines. Regeneration must be byte-identical;
   a scratch meaningful-line edit must fail proof, then restore the control.
3. Parent/candidate bake test lists remain identical (177 on planning base,
   including 13 soldier tests). Run `cargo test -p bake --lib -- --nocapture`,
   workspace build/fmt/all-target Clippy and rustdoc comparison. Missing-data
   early returns are not executed content checks. All 81 existing bake doc
   warning messages/multiplicities must stay unchanged or improve for a
   substantiated reason; no new unresolved links.
4. Compare synthetic public output calls on both versions: two nonempty
   texture files with exact retained/opaque alpha and duplicate suppression,
   and one named weapon node plus an absent-model control. Require the same
   three output files, all byte-identical. This fixture is not disc acceptance.
5. Both source files stay below 1,500 lines; update CRATES ownership/counts,
   inspect source references/test filters and run map/protect. No current
   soldier.rs size waiver exists. Historical line citations remain historical;
   cite moved functions by symbol. Preserve every original SPL14/SPLIT_RECIPE
   gate: full CI, actual-disc scratch-bake equality (SAME > 0, DIFF = 0,
   no missing/extra files), unchanged GPU/goldens and applicable sim, motion,
   sweep, arena, leak, parity and behavior gates. Investigate every move;
   never rebaseline. Neither this card nor SPL14 is done until these pass.
