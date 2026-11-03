---
id: ARVS
title: Arena head-to-head: tier X on one team against tier X-1 on the other (kill share, ticket margin, 24 seeds) - the arena today sets ONE tier for both teams, so its kills / min measures how deadly the fight is, not which tier wins (card TIER: Elite's extra movement when hit lowers BOTH sides' kills)
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
Card TIER (2026-10-19, report `loop/reports/20261019_1535_measure_TIER.md`): `arena` puts one `Sim::difficulty` on both teams. Above Normal the tiers do not separate on Tatooine 2 (24 seeds) and on the flat field Elite is BELOW Hard (kills / min 60.8 vs 77.5, hit rate 0.394 vs 0.465, 22 of 24 seeds), because Elite is harder to hit as well as a better shot: its REACT weights (moves more when hit), its view 1.5 and reaction 5 lower the OTHER side's hits too. A same-tier fight cannot show that Elite beats Hard; card TCMP's acceptance ("Easy < Normal < Hard <= Elite on kills a minute") is judged on this instrument.
## Notes
- planner 2026-10-19 16:25: value 4 -> 5 and TCMP `deps += ARVS`: TCMP's acceptance (the tiers separate) needs a referee that can see it, and TIER measured that the same-tier arena cannot (hard vs elite reversed on flat, t -7.3). Take the gate half too: replace the arena's one-seed `monotonic: kills` line with the head-to-head kill share (an 8-seed mean), or drop it as a verdict (TIER's gate proposal).

## Result (build 2026-10-20, branch arvs e7c3006, parked for land)
- `Sim::team_difficulty: [Option<Difficulty>; 2]` + `difficulty_of(team)` / `enemy_difficulty_of(team)`; every per-unit tier reader takes the unit's team (spawn skill, think tier/skill, err cap, spread, view, snipe sight, human damage by the shooter's team, BF2 aim blocks by the shooting side). Only the bleed handicap stays on `Sim::difficulty`. Hashed only when set. DIFB's teammate tier can use the same field.
- Null: gate `arena ... all` byte-identical (flat + tat2); `vs normal:normal` hash = `normal` row (5cf2b443a8dd085a flat, 6dba71520470191d tat2).
- Table (seeds 1-24, each pair both ways; lead_tmp/arvs/{flat,tat2}_vs_all_24.txt), higher tier's kill share / paired t / seeds won:
  flat: normal>easy 0.792 t +27.4 24/24; hard>normal 0.683 t +12.7 23/24; elite>hard 0.466 t -1.72 8/24 (coin flip, leaning Hard)
  tat2: normal>easy 0.714 t +61.4 24/24; hard>normal 0.557 t +12.6 24/24; elite>hard 0.515 t +3.35 16/24 (yes)
- Mutation (higher tier plays the lower): all six lines 0.500, t 0 (lead_tmp/arvs/MUT2_*). Flat seeds are weakly independent (seeds 1, 2, 4, 5 give the same rows) - its t is inflated; trust tat2.
- Gate half NOT done: the `monotonic:` line is kept (acceptance: the gate output stays byte-identical) and the module doc calls it a determinism row, not a verdict. Proposal for the planner: add `arena 9000 1 tat2 vs all 8` as a gate row (~25 s with ARENA_JOBS=12).
