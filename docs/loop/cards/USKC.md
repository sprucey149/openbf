---
id: USKC
title: CollisionRootScale: a soldier class's collision size (Wookiee 1.15, BF2 snipers 0.95, Jawa 0.6) - the port's capsule is one shape for every class
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Split from USKN (analysis/re-soldier-skeleton-skins.md §8 [O]). The PS2 sizes a soldier's collision primitives by its
class's `CollisionRootScale` s. BF1 `FUN_0020e2e0` (its only reader past the constructor, copy and setter, receipt
3f15c787) builds three stance sets:
- `coll_body`: length 1.55 s, at 0.8 s;
- `coll_head`: at 1.65 s, r 0.17 max(s,1);
- `coll_body_upper`: at 1.9 s - 0.42;
- the feet sphere is fixed.
BF2 `FUN_002517e8` does the same, its main capsule (1.65 s at 0.9 s) included. BF2 also reads s in `FUN_0024f0b0`,
`FUN_0024f350` and `FUN_00234a68`.

The port's soldier capsule is one shape for every class: r 0.5, h 1.9 / 1.6 / 0.55 (`core::unit::capsule`). It is used
by the hits, the walls and the spawns. So:
- the Wookiee (1.15; BF1 smuggler, 14 GCW missions; BF2 `all_inf_wookiee`, 35 scripts incl. UTA1 / MYG1 / HOT1) is hit
  and blocked as a human;
- the BF2 snipers (0.95) likewise;
- the Jawa (0.6) and Yoda (0.42) when they are fielded.

Also from §7, BF2's AI multiplies a point on a soldier target by the target's `SkeletonRootScale` (`FUN_00335920`,
`FUN_00337b80`); which point is [H].

## Evidence
- analysis/re-soldier-skeleton-skins.md §8: the primitive table with its lines, verified by the workflow (C12, C14
  SUPPORTED; A2 by two readers).
- The fielded values are in §8 (`lead_tmp/uskn/census_bf2_keys.txt`, `lead_tmp/unit-audit/census_all.json`).

## Acceptance
- First a short RE: which of the primitives the ordnance hit test uses (`coll_body` / `coll_head` against `main_body`),
  and which point BF2's AI scales. Paste the cited lines.
- Then build: the soldier's capsule height (and head point, if the port has one) per class x s, by the PS2's form.
  - A test: a 1.15 class's standing capsule top and the hit of a shot at 2.0 m height, against a human's.
  - The arena / sweep numbers declared (a gameplay change).
