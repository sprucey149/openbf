---
id: TIER
title: Do the four AI tiers order by kills? arena_tat2's one seed flips with every AI landing (kills / min easy, normal, hard, elite: AIRC 18.0 23.4 26.2 28.6 yes; OFFG 16.0 25.6 26.6 24.4 no; STLVL 20.0 22.4 23.6 28.8 yes; STTAP 17.2 26.0 24.4 25.0 no) - measure 8 seeds per tier before anyone tunes a tier
milestone: M3
value: 5
phase: measure
status: done
deps: CONT,BOTD,TPOS,GRAI
files:
declares:
needs:
effort:
branch:
resume:
created: 2026-10-02
---
## Why
The difficulty setting is the one AI knob the maintainer will turn with a controller, and the only thing that checks it is the
arena gate's `monotonic:` line: one 9,000-tick run per tier on one seed. Two reports in a row put "kills are no longer
monotonic by tier" under For the maintainer (OFFG, STTAP) and nobody owns it. Planner 2026-10-02 09:30.

## Evidence (`arena 9000 42 tat2 all`, line 3-7 of `arena_tat2_1.txt` in each baseline dir, read by the planner)
| baseline (`lead_tmp/loop_baselines/`) | easy | normal | hard | elite | `monotonic: kills` |
|---|---|---|---|---|---|
| 20261002_0354_AIRC, 0422_LVLF | 18.0 | 23.4 | 26.2 | 28.6 | yes |
| 20261002_0513_OFFG | 16.0 | 25.6 | 26.6 | 24.4 | no |
| 20261002_0705_STLVL (main's baseline now) | 20.0 | 22.4 | 23.6 | 28.8 | yes |
| branch sttap (`lead_tmp/sttap/gates/`) | 17.2 | 26.0 | 24.4 | 25.0 | no |
`hits` reads `no` in every one of them (hit rate easy 0.12-0.15, the other three 0.18-0.24 in no fixed order).
The order flips with each landing, so one seed cannot say whether the tiers differ at all above `easy`.

## Notes
- No `files:`: a measurement with the gate's binary. Do not tune a tier in this card.

- Planner 2026-10-02 14:45: `deps: CONT`. CONT's build replaces the commander's scoring; eight seeds per tier measured before it lands would be stale the same day.

## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

## BOTD (2026-10-03) - stays the measurement; its build is card TCMP (bot-design.md section 7)
Run it on main BEFORE TCMP changes a tier. In BOTV's deps.
- 2026-10-05 (the maintainer, via Claude): the difficulty design now lets the humans' AI teammates sit on a LOWER tier than the enemy's bots (cards DIFH / DIFS / DIFB). Measure the four tiers so that a team can carry a tier of its own, and report the shooting columns (aim error, first shot, bursts) separately from the movement and position columns.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decisions 2 and 8 are card TCMP's (no damage bonus at Hard / Elite, one think rate): measure today's tiers WITH the bonus still in, so TCMP's change is a before / after.
- 2026-10-05 planner: value 3 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set TIER value=3`. `deps` gains TPOS, GRAI: the design's build order (bot-design.md 12, row 13) takes this measurement right before TCMP, on the bots TCMP will change, not on today's. Undo: `set TIER deps=CONT,BOTD`.
- 2026-10-07 (the maintainer, via Claude): the reaction to a hit is a tier ladder (card REACT): write the stay / strafe / roll / new-position weights per tier next to the evasive-step and roll-when-hit columns, so Easy mostly stays and Elite mostly moves.

## RESULT (measure, 2026-10-19, main 7752d00, lane 2; report `reports/20261019_1535_measure_TIER.md`)
`arena 9000 <s> tat2 all` and `arena 3000 <s> flat all`, seeds 1-24 (three blocks of 8); seed 42 byte-identical to the gate
baseline (20261019_1345_TURR). Runs: `lead_tmp/tier/runs/{s,f}<seed>.txt`, aggregate `lead_tmp/tier/agg.py`. Both teams carry
the SAME tier in this instrument, so a row says how deadly a same-tier fight is, not which tier wins.

tat2, 16 v 16 (mean, seeds 1-8 min-max):
| tier | kills/min | hit rate | ttfk s | shots/kill |
|---|---|---|---|---|
| easy | 29.4 (27.6-32.8) | 0.202 (0.187-0.209) | 16.6 | 33.7 |
| normal | 37.7 (35.2-39.8) | 0.267 (0.246-0.277) | 16.2 | 23.8 |
| hard | 38.3 (34.2-42.4) | 0.262 (0.232-0.275) | 14.4 | 24.2 |
| elite | 37.2 (33.6-40.2) | 0.263 (0.243-0.286) | 13.9 | 23.5 |
flat 8 v 8 one life: kills/min 29.4 / 62.3 / 77.5 / 60.8, hit 0.290 / 0.434 / 0.465 / 0.394, ttfk 3.7 / 1.24 / 0.92 / 1.44 s.

Verdicts (paired over 24 seeds):
- easy < normal: APART on both fields (tat2 +8.3 kills/min, t +17.6, 24/24 seeds; hit +0.065, t +20).
- normal < hard: INSIDE THE NOISE on tat2 (+0.57 kills/min, t +0.9, 12/24; hit -0.005); apart on flat (+15.2, t +6.2, 22/24). Only ttfk separates on tat2 (-1.8 s, t -3.5).
- hard < elite: REVERSED. tat2 inside the noise (-1.07, t -1.4); flat clearly reversed (-16.7 kills/min, t -7.3, 2/24; hit -0.071, t -6.6).
Why hard / elite: both are base skill 20, so the whole skill-curve vector (spread 1.0, dispersion 0, aim accel 14, floor 0.4 m, burst, pause, tap) is IDENTICAL; only `Tier` differs: err_cap 0.5 vs 1.2 m, view 1.5 vs 1.25, reaction 5 vs 6 ticks, react [.30,.25,.35] vs [.30,.20,.30], grenade 0.95 vs 0.85 (pressure, human_damage, bleed: humans only). Knock-outs on Elite (scratch, flat, 24 seeds, `lead_tmp/tier/runs_{react,cap,view}`, `ko.py`): Hard's react: kills/min 60.8 -> 66.4 (t +2.7), hit 0.394 -> 0.415; + Hard's view / reaction: -> 72.9, hit 0.441, ttfk 1.44 -> 0.92 (= Hard's); + no err_cap instead: -> 66.9, hit 0.430. So Elite's extra MOVEMENT when hit and its wider view (a slower first kill on flat: 1.44 vs 0.92 s) make the same-tier fight less deadly; its tighter 0.5 m cap is not worth the rest. Normal (17) vs Hard (20) is t 0.842 vs 1.0: spread 1.32 vs 1.0, dispersion 0.32 vs 0, aim accel 8.7 vs 14, floor 0.59 vs 0.40, cap 1.36 vs 1.20 - too small a step to show on tat2.
Shooting columns per tier (skill 8 / 17 / 20 / 20): spread_mult 2.26 / 1.32 / 1.0 / 1.0, dispersion 1.26 / 0.32 / 0 / 0, aim accel 1.46 / 8.68 / 14 / 14, err floor 1.16 / 0.59 / 0.40 / 0.40 m, err cap 1.83 / 1.36 / 1.20 / 0.50 m, burst_t 0.37 / 0.84 / 1 / 1, pause 1.21 / 0.83 / 0.70 / 0.70, first shot (reaction) 15 / 9 / 6 / 5 ticks. Movement / position columns: view 0.8 / 1.0 / 1.25 / 1.5, react stay-strafe-roll-move 65-20-5-10 / 35-30-15-20 / 20-30-20-30 / 10-30-25-35, grenade run 30 / 60 / 85 / 95 %, dodge 0.074 / 0.168 / 0.2 / 0.2. The PS2's bases are BF2 4 / 14 / 17 (non-human teams), so the port's Hard and Elite sit above the PS2's top.
Follow-up: the build is card TCMP (note added there: which fields separate the tiers); its acceptance needs a head-to-head instrument - new card ARVS (value 4). Gate proposal in the report: the `monotonic:` line should not be a one-seed same-tier verdict.
