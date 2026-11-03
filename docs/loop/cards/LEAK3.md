---
id: LEAK3
title: The leak census misses walls a soldier meets, and its gate accepts any rise once a card declares leak (AUDIT of LEAK): sample every SOLDIER wall at body height along its extent (not one centroid per face; sphere belts; the live world's parked hulls), list every map x class count that rose and fail the ones a card did not scope, and let 'new' fire
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT of LEAK (2026-09-30, PARTLY). The census is deterministic, and a gross regression moves its bytes. But:
- It measures only part of what a soldier can reach. Only 18.6 % of its side-samples become runs. A sampler that uses
  the same runs and classifier, but places samples at body height along every wall, finds about twice as many leak
  sites.
- Its gate cannot tell a fix from a new hole once a card declares `leak`, and every collision card does.
- `new` can never be non-zero.

Every alpha "no soldier out of bounds" card (VCOL, WPSR, WTHIN, LEAK2) is measured with this census.

## Evidence
Report `engine/artifacts/loop/reports/20260930_0930_audit_LEAK.md`; scratch `engine/artifacts/lead_tmp/audit-leak/`
(`leak_audit.rs` = the census + skip-reason counters + `AUDIT_SAMPLER=band`; one Opus verifier: 11 SUPPORTED,
4 PARTLY on wording, 0 NOT).
1. **Sampling.** `wall_samples` makes one sample per triangle, at its centroid; box sides at their centres; 8 cylinder
   points at mid-height; spheres none.
   - A side is skipped when the floor 0.8 m out is more than 2.5 m below the sample.
   - On main that skips 319,438 of 658,346 side-samples. 124,797 of those are faces that do span the soldier's
     0.8-1.8 m band over that floor. That is more than the 122,271 starts actually run.
   - Example: BF2 NAB2's 64 m blocks 562/563/584 are never driven at (side centres 26 m over the street).
2. **The band sampler** (`band_samples`): points on every SOLDIER wall face at 0.8-1.8 m over the floor beside it,
   every 1 m, spheres included.
   - Sites: 27,035 against the census's 14,597.
   - 7,253 band sites have no census site within one cell. 508 go the other way (`cmp_sites.py`).
   - Band-only by class: push 112, tunnel 154, offmap 39, ceiling 2,446, floor 1,469, closed 2,961, buried 49, float 23.
   - Showcase maps, band-only sites: UTA1 379, TAT1 203, YAV2 147, HOT1 99, MYG1 88, NAB1 18.
3. **Gate.**
   - The accepted list is 8 class wildcards, so `new` is identically 0. The `leak_new` row fires only on a crash or a
     missing list.
   - A declared `leak` re-baselines every count, and nothing lists which map x class rose (the sweep has
     `sweep_moved_rows.txt`).
   - Each of the three `leak` landings raised counts: JWAL 36 map x class counts (bf1_bes2 push +698), WPSH 11
     (bf2_kas2 tunnel 15 -> 49 with the start rule held fixed, unreported), WTUN 41 (closed +2,024).
4. **Regressions.** A hook drops a primitive kind from the wall pass: `OPENBF_AUDIT_REGRESS` in walls.rs, worktree
   `openbf-wt/audit-leak-regress-scratch`.
   - The census moves: cylinders 32/32 maps, spheres 3/32 (bf2_dea1's 4 spheres unseen), boxes 32/32, boxes > 20 m
     14/32.
   - The band sampler shows 18x the census's signal on the > 20 m boxes (+293,571 against +16,161 leak runs).
   - `new` stayed 0 every time.
5. **No vehicles.** `build_sim` never calls `vehicle_pads::install`, so no parked hull (VHULL) is in the census's world.

## Acceptance
- **Coverage.** The census samples walls where the soldier meets them: at body height along each face, sphere belts,
  and the live world's parked hulls (`vehicle_pads::install`, as the sweep does).
  - On the landing's parent, band-only sites against the new census are ≤ 5 % of the band sampler's (audit: 27 %),
    and the run takes ≤ 6 min on 32 threads.
  - The CRATES.md row and the census header state what it covers.
- **Gate.**
  - gates.py's leak group writes `leak_moved_rows.txt` (each map x class old -> new) and prints the rises.
  - A declared `leak` names what may rise (e.g. `leak=<map>:<class>,…`, like `parity=`). Any other rise FAILS.
  - The accepted list pins per-map counts or sites, so a new hole makes `new` > 0.
  - Test: the audit's hook (`cyl` / `sphere` / `box` / `tallbox`) FAILS the gate naming the maps, even with `leak`
    declared for one other map.

## Notes
- land.py runs main's gates.py (memory `land-runs-mains-gates-script`). Land the gates.py/land.py half first, then
  the sampler.
- The sampler change moves every count: re-baseline once, with `leak` declared and the moved rows viewed.
- LEAK2's map-edge work starts from the NAB2 edge exits this sampler finds (note on LEAK2).
- 2026-10-06 planner: `engine/CRATES.md` taken out of `files:`. The picker refuses a card whose files overlap a card in progress, and 22 open cards listed the map, so any one of them held the other 21 (a chain card in progress held WADE and WTHR). The map's row for a new or moved source file is still owed in the same branch (`scripts/size_report.sh --check-map`); a rebase conflict in it is one line. Undo: add the path back.

## 2026-10-22 lane 1 (build, part 1): split
- Built: per-map `pin` lines in leak_census_accepted.txt cap what a class line accepts; each map's site line lists sites
  (and new) by class; `LEAK_PIN_OUT` writes the pins. Branch leak3 2e654cd. Mutation: bf1_kas2 push pin 9 -> 7 = new 2.
- The gate half went to **LEAK3G**: scripts/gates.py / land.py are oracle-guarded (hook refused the edit); draft + test in
  lead_tmp/leak3/. The sampler half went to **LEAK3S**. This card's Acceptance "`new` > 0 on a new hole" is met here;
  "Coverage" moves to LEAK3S, "Gate" moved rows / scoped declare to LEAK3G.
