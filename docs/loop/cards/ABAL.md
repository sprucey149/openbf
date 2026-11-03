---
id: ABAL
title: BF2's AI auto-balance: in a single-player or co-op round the PS2 shifts the two teams' bot SKILL by the score gap (enemy AI +1 per 10 points past the threshold, at most +-4 of 20, the human team's AI the opposite; re-ai-difficulty 6, seen live 14 -> 17 / 11 -> 8 in 240 s on Polis Massa) - the port has no such shift
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
Filed by the planner 2026-10-12 from card CLSE2's report (reports/20261009_1753_card_CLSE2.md, 'For the planner'): the PS2's
auto-balance is LIVE in a single-player round and the port has none; no card existed.

## Evidence
- The rule, read and verified: analysis/re-ai-difficulty.md section 6 (FUN_002a3528 every 15 AI ticks: diff = human score - enemy
  score, +20 / +10 when the human team's tickets are under 25 / 50 % of the start; past the threshold (15 with under 1000 tickets a
  side) the enemy AI's delta = (diff -+ threshold) / 10 clamped to +-4, the human team's AI the negative; off in netplay and
  split-screen versus; it never changes bot counts). Mode 1 also hands a unit of the losing side a new goal on a timer (FUN_002a3c28).
- Seen on the running PS2: analysis/re-soldier-ai.md 18.1 - the skill behind the error floor stepped 14 -> 17 on one team and
  11 -> 8 on the other in 240 s on Polis Massa (card CLSE2's capture, engine/artifacts/lead_tmp/clse2/).
- The port: no hit for auto-balance under engine/crates/core/src (grep 2026-10-12).

## What to do
1. Check which of the port's per-unit numbers the PS2's skill feeds (skill.rs's tier rows against re-ai-difficulty 7 items 1-6) and
   whether BF1 has a twin (re_check.py callers; section 6 is headed BF2).
2. Build the delta in the sim (deterministic, hashed), applied where the tier's skill is read; off for a round with no human and
   for versus split-screen, as the PS2.
3. FEEL_NOTES line; DIFB / DIFS (the maintainer's handicap, waiting on him) stack on top of it - say how on those cards.

## Acceptance
- A unit test per branch of the rule: the gap at the threshold gives 0, threshold + 10 gives +1 / -1, 55 past gives +4 / -4, the
  ticket bonus at 25 % and 50 %, off with two human teams; each fails under its own mutation.
- A replay check against the PS2: a port round driven to CLSE2's Polis Massa score trace gives the same delta steps (14 -> 17,
  11 -> 8) within one step at the same times.
- 8 seeds x 32 maps: 256 / 256 rounds finish; with no human in the round the sweep is byte-identical to main (the rule is off).

## Notes
- Value 3 and not alpha (planner 2026-10-12): faithful, but nobody has asked for it and the bot programme's fives come first.
  Raise it if the maintainer wants the PS2's rubber band in his playtests. Depends on CLSE2 because that card puts the skill-fed error
  terms in (gunner_foot.rs); files: left empty so it holds no AI file while it waits.
