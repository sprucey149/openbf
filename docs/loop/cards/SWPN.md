---
id: SWPN
title: The round sweep over 8 seeds: per map, the finish rate and the stuck_max distribution, so a stall fix is judged by more than seed 42 (the maintainer's STALL step 1)
milestone: M3
value: 5
phase: build
status: done
deps:
files: scripts/sweep_seeds.py,engine/crates/game/examples/round_to_outcome.rs
declares:
needs:
branch: swpn
resume: landed bcfb2e2d 2026-09-30
created: 2026-09-30
alpha: yes
---
## Why
The maintainer (INBOX 2026-09-30 08:50, STALL step 1): the sweep reports one seed, and one seed is a noisy instrument — WTUN's
seeds 1–8 showed KAM1's seed-42 win was the lucky run (7/8 NO OUTCOME), POL1 6 → 8 of 8, MUS1 1 → 2
(`reports/20260929_2337_card_WTUN.md`, `lead_tmp/wtun/reseed/`). STALL's fixes are judged by the multi-seed numbers.

## Note from STALL (2026-09-30)
STALL's scratch run gives SWPN a first baseline: main 027dad9, seeds 42 and 1–8, all 32 maps, 60,000 ticks.
- **The files:** `lead_tmp/stall/all/sweep_s*.txt` and `eps_s*.err`.
- **The per-map finish table:** `analysis/re-round-stalls.md` §0.
- **Wall time:** 32 maps × 9 seeds at 32 threads under ci.lock took ~9 min. The seeds took 44–210 s each: a finished
  map is quick, a 60,000-tick stall costs ~40 s.

The scratch census (`lead_tmp/stall/tools/stall_census.patch` on `round_to_outcome`) is read-only. Seed 42 still matches
the baseline hashes. It adds:
- `STALL_LOG` episodes;
- `DEATHS`, `SHOTS`, `AMMO`;
- `TRACE_UNIT`.

**Worth folding into SWPN's tool:**
- the per-map deaths (kills vs environment) — KAM1's 99 % environment is the clearest single number;
- the no-progress episodes, which the `stuck_max` footer misses.

## Result (2026-09-30, lane 2)
- `scripts/sweep_seeds.py` + `round_to_outcome`'s `SWEEP_DEATHS=1` (stderr only). Main's baseline, seeds 1–8:
  `lead_tmp/loop_baselines/20260930_1600_SWPN/seeds.md` (with seed 42: `seeds9.md`). Wall time 118 s for all 9 seeds at once.
- Instrument: seed 42 byte-identical to the gate's `sweep_main_1.txt` (32/32 rows); all 9 seeds' stdout byte-identical
  to STALL's `lead_tmp/stall/all/sweep_s*.txt`; the deaths lines match STALL's DEATHS census on all 288 map × seed rows (other = vehicle + blast).
- Footer (1–8): maps finishing >= 7/8: 27/32; any stuck_max >= 10k: 6 (B1 KAS2, B1 YAV1, KAM1, MUS1, MYG1, POL1);
  rounds finished 227/256. KAM1 0/8 (99 % world deaths, all region), POL1 0/8, DEA1 4/8 (77 % world).
