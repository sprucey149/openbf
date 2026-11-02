---
id: HYST
title: Position hysteresis (keep the held spot unless another is 15 % better, clamped score terms), no cover reached by running at the enemy, and a hit / near miss / new enemy in sight wakes a bot at once
milestone: M3
value: 5
phase: done
status: done
deps: ARCM
files: engine/crates/core/src/ai/tactic.rs; engine/crates/core/src/ai/tactic/tests.rs; engine/crates/core/src/ai/wake.rs; engine/crates/core/src/ai/wake/tests.rs; engine/crates/core/src/ai.rs; engine/crates/core/src/ai/behave/brain.rs; engine/crates/core/src/ai/behave/position/tests.rs; engine/crates/core/src/ai/state.rs; engine/crates/core/src/ai/why.rs; engine/crates/core/src/sim/state.rs; engine/crates/core/src/sim/step.rs; engine/crates/game/examples/round_to_outcome.rs; analysis/bot-design.md
declares: sim
needs:
effort:
branch: botup
resume:
created: 2026-10-28
---
## Why
`lead_tmp/botre/modern.md` B1-B3 (its recommendations 4 and 5): the Tactician's 5-12 s timer and the open
position's 2 s ask for cover could swap between two near-equal positions (`analysis/bot-design.md` 4.6 scored each
term 0-1 without holding it there: an authored arrow's +0.2 took LOW cover to 1.2); nothing stopped a bot from
running at its enemy in the open to reach the best-scoring cover (The Last of Us' "NPCs rush forward to take
cover"); and a bot hit, shot past or shown a new enemy just after a think answered only at its next think (up to
0.33 s later).

## Evidence
6 seeds (42-47) x 34 maps, `round_to_outcome 60000` with `STUCK_CENSUS=200 SWEEP_DEATHS=1 HIT_CENSUS=1`,
`lead_tmp/botre/runs/HYST/seeds/`: `off` = this build with every rule switched off by a measurement-only switch
(its stdout byte-identical to ARCM's `final` on all six seeds, so it IS the before), `final` = this commit; the
variants `nowake`, `noclose`, `noup` (no hysteresis on the open spot's ask), `close8` (an 8 m closing alone) were
measured on the way and are kept beside them. Bot quality (`scripts/bot_quality.py`, BOT_SAMPLES 540 s, seeds 42-43,
all maps) for the reversals.

| | before (off) | after (final) |
|---|---|---|
| finished | 204/204 | 204/204 |
| stuck_max per map: median / max (ticks) | 120 / 360 | 120 / 360 |
| recycles | 19 | 19 |
| time in Recover (of foot time) | 1.47 % | 1.36 % |
| unexplained standing (stand20, foot-s) | 9,164 | 8,389 |
| position flips (a held position replaced on one think) per fight-minute | 2.02 | 1.71 |
| queries whose winner did not beat the held position by 15 % (kept) | - | 3,855 |
| B12: fight time at a position with cover from the target | 15.27 % | 15.61 % |
| fight time at a Tactician's position | 37.5 % | 38.6 % |
| contact20 (an enemy within 20 m) | 13.81 % | 13.44 % |
| kills per foot-minute / mean kill imbalance between teams | 1.181 / 0.082 | 1.196 / 0.088 |
| team kills / world deaths | 468 / 236 | 462 / 250 |
| reversals per bot per 10 s / pinball windows / B9 (bot_quality, 2 seeds) | 0.61 / 0.94 % / 0.28 % | 0.59 / 1.06 % / 0.20 % |
| early thinks: by a hit / a near miss / a new enemy in sight | - | 152,087 / 531,398 / 18,281 |
| behaviour changes those thinks made (WHY codes) | - | seen 8,563, hit 1,955, target_lost 408, claim 294, grenade 285, ... |
| census wall seconds summed over maps | 1,361 | 1,364 |

The closing rule alone first used 4 m (`full`): it took 1 covered candidate in 7 out (x-closing 276,671), B12 fell
15.27 -> 14.79 % and contact20 13.81 -> 12.79 %; 8 m alone (`close8`) still 14.96 %. With the share of the distance
(`final`) it bites in close fights only: B12 15.61 % against 15.68 % with no closing rule (`noclose`). Without the
wake (`nowake`) stand20 stays 9,059: the wake is what takes unexplained standing down. Seed noise on recycles is
11-28 across the variants.

## Change
- `tactic::choose` takes the position held (`keep`): the timer's query and the open position's 2 s ask score it as
  it stands with the candidates' own score, and a winner must beat it by `SWITCH_MARGIN` = 15 % (a tie keeps it;
  `HY_KEPT`). The score is one closure for both; each term held to 0..1 (an arrow's +0.2 no longer lifts LOW cover
  over 1). Ties between candidates go by score, then table index (unchanged; the per-unit bias stays the "side").
- A fourth filter (8, the census's `x-closing`): from inside the band, a position whose walk closes on the target
  by over `CLOSING` = 4 m and over `CLOSING_SHARE` = 40 % of the bot's distance to it, heading at it (the closing
  over half the walk), is not taken. Outside the band the bot has to close and may.
- `ai::wake`: a hit since the last think (`last_hit`), an enemy bolt whose last step passed within 2 m of the
  body (feet to eye), or an enemy within 20 m in view and in the line of the eye that the bot holds no record of,
  sets `AiState::wake`; the bot thinks at the next tick, at most every `GAP` = 3 ticks (0.1 s). The next scheduled
  think keeps the unit-index phase. The tier's first-shot delay is untouched (`reaction_ticks`). `last_think` and
  `wake` are scheduling, not hashed (as `next_think`).
- Census `hyst <key>: flips N kept N | wakes hit N near N seen N | wake changes [why n ...]`, `tfunnel ... x-closing`.

## Deviation
D21 (`analysis/bot-design.md` 9): the PS2's BF2 bot thinks at the next step on a threat entry for a HUMAN unit only
(`re-ai-think-rate.md` 4), BF1 never; ours wakes on any hit, near miss or new enemy in sight. The PS2's timer takes
"another qualifying hint"; ours keeps the held one unless another is 15 % better. The PS2 has no closing rule.

## Open
- The near-miss wake fires about 3.5 times as often as the hit wake; it is bounded by the gap, and the per-think
  draws (the circle stop's chance per think, BF1's tracked error) now run a little more often under fire.
- Sight is looked for each tick within 20 m only; a new enemy farther off still waits for the think's scan.
- The §7 table's slower think at Easy / Normal is still not built (every tier thinks every 0.33 s); when it is, the
  wake keeps reactions at the first-shot delay.
