---
id: RBKM
title: A manifest schema change must rebake every manifest-writing group: rebake=maps skips tat1/tat2 (own groups), so REFL left their manifest.ron without reflect/reflection_color and land.py's maps-only bake gate could not see it — expand or refuse
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
`engine/CRATES.md` says a new manifest field means rebaking every group that writes a manifest. `scripts/rebake.sh`'s
`maps` group bakes the 30 other ground maps but NOT `tat1`/`tat2` (their own groups, rebake.sh:74-83, :235). A card that
declares `rebake=maps` for a manifest change leaves the two Tatooines stale on main, and land.py's bake gate runs
`scratch_bake.sh maps` (ONLY_IN_B=16968 = everything else unchecked), so nothing notices.

## Evidence
Cleanse 2026-09-29 (report `reports/20260929_0439_cleanse_manifest.md`): `gates.py --auto` on the comment-only branch
cleanse-manifest showed bake DIFF=5 — `tat1/manifest.ron`, `tat2/manifest.ron` (missing REFL's `reflect: false` and
`reflection_color: None`; main's files dated 02:42, REFL landed 03:02 with `declares: … rebake=maps`) + 3 ffmpeg OGGs.
The cleanse rebaked tat1 tat2 on main to repair the drift.

## Acceptance
- land.py (or parse_declares) expands `rebake=maps` to `tat1 tat2 maps` — or refuses a branch that touches
  `engine/crates/manifest/` or a manifest writer in `bake/` unless the declared groups cover every manifest-writing group
  (tat1 tat2 maps common side). Pick one; a self-test in land.py/gates.py shows it.
- A land run's main-side bake gate compares every group it rebaked.

## Notes
