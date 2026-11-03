---
id: SOLLO
title: Soldier _low1 LOD and far-scene soldiers: ~3 hi-res bodies + the player, the rest _low1; past the near scene the 10 nearest soldiers as _low1 to FarSceneRange (re-lod.md, never built); presentation only
status: archived
issue: 69
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #69](https://github.com/sprucey149/openbf/issues/69).

## Why
The maintainer 2026-10-27 (INBOX). openbf's own research, never built:
`analysis/re-lod.md` §0.5, §4.1-4.2, §5; LOD.md:64-65 named it. No `_low1` / GeometryLowRes use in engine/crates today.
The PS2 draws ~3 hi-res soldiers plus the player and the rest as `_low1`; past the near scene at most 10 soldiers, nearest
first, as `_low1` out to FarSceneRange (check what openbf draws past the cull today first).

## Acceptance
- Strict mode = the PS2 rule from `re-lod.md` (budgets, order, ranges), re-checked against PCSX2 pairs on one BF1 and one BF2 map.
- Classic-plus default: may keep more hi-res bodies on a strong GPU; the count is chosen from PERF.md on the maintainer's machine and
  the mid-class-laptop proxy, and PERF.md records the gain at 16 and at the slider's max bots.
- Presentation only: sim hash unchanged; goldens updated where the far soldiers now show.
