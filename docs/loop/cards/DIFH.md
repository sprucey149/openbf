---
id: DIFH
title: Difficulty test harness and the first lever curves: a headless battle takes N stand-in humans per side and four levers (teammate shooting tier, the enemy's units on the field, reinforcements, the enemy's spawn-wave time); stage 1 = each lever alone on six maps with one human, win rate and ticket margin per step
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `todo`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
The maintainer, 2026-10-05: "You turn up the difficulty so the ai gets better so the user is more likely to die, how does that translate to making it harder to win games? If all ai is better including teammates, and the user just dies more, how does that make it harder to win a match? ... We need a good solution where victory is somewhat tied to user performance on any difficulty. Easier difficulties are a little more evenly split, while harder difficulties depend on the user playing well and thought needs to be incorporated for when multiple users are playing and they're on opposing teams." With equally good bots on both sides a battle is a coin flip the human tips; better bots only shrink the tip.

The design agreed in chat (all PROVISIONAL until DIFS's numbers are in front of the maintainer):
- Difficulty still sets how good the bots are. On top of it a HANDICAP against the side with more humans, sized by the difference in humans (1 v 0 full, 2 v 0 double, 1 v 1 none, 2 v 1 one human's worth to the lone player's side).
- Levers, used in this order so no single number gets silly: (1) the humans' AI teammates shoot worse than the enemy's bots - the maintainer: "if 4 players are on one team and playing elite, the user teammates maybe use the easy ai intelligence, while enemies use the elite" (the PS2's own lever: BF2 teammates 6 / 11 / 11 against 4 / 14 / 17). Claude's caution, for the test to settle: lower only the SHOOTING columns (aim error, first shot, bursts), not how sensibly a teammate moves and takes cover; (2) more enemy units on the field - the maintainer: "16 per side ... a harder difficulty would bump the enemy up to say 17 or 18"; (3) reinforcements; (4) the enemy's spawn-wave time, as a fourth lever to measure.
- The maps' own lopsidedness STAYS - the maintainer: "I kind of like that in some maps - the difficulty or artificial difficulty of different maps change even under the same difficulty. I think that is fun so I don't want to lose that." The handicap is tuned on the average over maps and added on top of each mission's own numbers. Seven of the 57 conquest missions are lopsided on the disc (engine/missions, read 2026-10-05): BF1 KAM1 tickets 220 rep / 200 cis; BF1 KAS1 and KAS2 (both eras) 32 / 25 units; BF1 GEO1 32 rep / 25 cis; BF1 NAB2 22 all / 25 imp.
- Nothing is shown on screen - the maintainer: "No need to show handicap - it just adjusts reinforcements and units when players join the game and choose a side."
- Caps - the maintainer: "I'd want to see where things start to get absurd and if we should put more caps on things so like if 16 users are on the same team the reinforcement numbers isn't say 200 vs 1000 or something crazy. I'd want to see the numbers though before I decide." 

## Evidence
- `scripts/sweep_seeds.py`, `round_to_outcome`: the headless round and its 8-seed sweep exist; neither takes a human stand-in, a lever or more than one human.

## Acceptance
- The harness: humans per side (0-16), the stand-in kind per human, the tier, and each lever as a run key; output per round: winner, ticket margin, time, each human's kills / deaths / captures.
- Stage 1: each lever alone in steps, one average and one strong human, six maps (two of the disc's lopsided ones among them), the four tiers, 8+ seeds: win rate and ticket margin per step, as CSV and one sheet per lever.
- The measure that answers the maintainer's question, reported for every cell: the win rate of the average stand-in, of the strong one, and the GAP between them ("victory depends on how well you play").
- What one unit of each lever is worth in tickets, so the levers can be traded.

## Notes
Filed 2026-10-05 by Claude in chat from a design talk with the maintainer. His words are quoted; no number here is measured. Today's bots are fine for building the harness and reading the curves' shapes; the final numbers are DIFS's, after BOTV.
