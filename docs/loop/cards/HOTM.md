---
id: HOTM
title: BF2 Hoth after MOUTH: bots are within 1.5 m of a team-mate 10.3 pct of their time over 16 seeds (5.5 before, up in 11 of 16 seeds) - find which of MOUTH's two rules does it and where, then fix it or show it is harmless
milestone: M3
value: 5
phase: measure
status: done
deps: MOUTH
files:
declares: sim; sweep; arena
needs:
effort:
branch:
resume: DONE 2026-10-12: the corner rule (MOUTH=1) - 48 seeds 5.11 -> 10.33, t +6.1; all of it is guards piled on two defend spots at post 1 (on guard 1.83 -> 7.05, the rest 3.28 -> 3.28); the pile is on main before MOUTH (defend_spot has no reservation); fix = card DEFS. Report reports/20261012_1622_card_HOTM.md, data lead_tmp/hotm
created: 2026-10-12
alpha: yes
---
## Why
Found by the planner's baseline re-take (2026-10-12 14:20): on BF2 Hoth, a showcase map, bots are within 1.5 m of a
team-mate about twice as much of their time since MOUTH landed (273a441). MOUTH's landing gate called it noise - seeds
9-16 stayed inside the per-map tolerance (6.5) - but over all 16 seeds it is not.

## Evidence
- `mate_1p5_pct@bf2_hot1` per seed (`gates._botq_one` over each baseline run's `botq/bf2_hot1_s*.csv`):
  | main | seeds 1-8 | seeds 9-16 | mean |
  |---|---|---|---|
  | 19708fa (before OBST2's cost cuts, DAGS, MOUTH) | 3.4 3.9 4.8 12.7 2.0 2.3 2.9 10.5 | 3.3 3.3 3.3 8.4 7.3 5.3 8.9 6.0 | 5.5 |
  | 4ca302f (DAGS's landing run) | 7.4 4.9 10.8 2.4 2.8 5.3 2.8 6.3 | - | 5.3 |
  | d096ae7 (MOUTH on) | 9.5 14.3 7.6 20.5 5.3 12.1 23.4 10.3 | 7.0 10.7 7.8 14.7 6.0 4.6 6.6 4.3 | 10.3 |
  Against 19708fa: up in 11 of 16 seeds, mean difference +4.8, paired t about +3.2.
- Dirs: `engine/artifacts/lead_tmp/fidelity_rebase_20261012b/gates/` (19708fa), `lead_tmp/dags/land/main/fidelity/`
  (4ca302f), `lead_tmp/fidelity_rebase_20261012c/gates/` (d096ae7); `fidelity/` and `fidelity_confirm/` in each.
- All maps together did not move (5.80 -> 6.08 on seeds 1-8, 5.97 -> 5.81 on 9-16): it is Hoth's.
- MOUTH's knob binary splits its two rules: `lead_tmp/mouth/build/` (`rto_knobs2`: `MOUTH=1`, `HOPWIN=1` on 4ca302f;
  null = main; use `env VAR=1 cmd` behind `timeout` / `xargs`).

## Notes
- The baseline now holds main's own value (12.87 on seeds 1-8, 7.72 on 9-16), so the gate does not refuse other cards on
  it; this card is what brings it down.
