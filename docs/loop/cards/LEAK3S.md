---
id: LEAK3S
title: The leak census samples walls where a soldier meets them (LEAK3's sampler half): body-height points along every SOLDIER wall face every 1 m, sphere belts, and the live world's parked hulls (vehicle_pads::install) - band-only sites <= 5 % of the audit's band sampler, <= 6 min on 32 threads
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
LEAK3's AUDIT (reports/20260930_0930_audit_LEAK.md, Evidence 1, 2, 5): one centroid per face; 124,797 skipped side-samples span the soldier's band; the band sampler finds 27,035 sites vs 14,597 (7,253 band-only; showcase UTA1 379, TAT1 203, YAV2 147, HOT1 99, MYG1 88, NAB1 18); no sphere samples; build_sim never installs parked hulls.

## Evidence
The audit's sampler: engine/artifacts/lead_tmp/audit-leak/ (leak_audit.rs, AUDIT_SAMPLER=band; cmp_sites.py; a_band 345 s on 32 threads). LEAK3 part 1 (pins) is on main: re-seed the pins with LEAK_PIN_OUT after the sampler change.

## Acceptance
- On the landing's parent, band-only sites against the new census <= 5 % of the band sampler's (cmp_sites.py), census <= 6 min on 32 threads.
- Parked hulls in the census world (vehicle_pads::install, as the sweep does); sphere belts sampled.
- The census header and CRATES.md row state what it covers; pins re-seeded; declares leak (every count moves; view leak_moved_rows.txt once LEAK3G is in).

## Notes
LEAK2's map-edge work starts from the NAB2 edge exits this sampler finds.
