---
id: DIFS
title: The difficulty numbers for the maintainer to decide on: every human split up to 8 v 8 and 16 on one side, the four tiers, all 57 conquest missions - the recommended lever settings per scenario, where they turn absurd, and the caps proposed (nothing is built into the game from this card)
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
- DIFH's lever curves and exchange rates.

## Acceptance
- Stage 2: every human split a v b with a >= b up to 8 v 8, plus 12 v 0 and 16 v 0, average and strong stand-ins, the four tiers: the lever settings that hit the targets, in the fixed order (teammate tier, enemy units, reinforcements).
- Targets to tune to (Claude's proposal, the maintainer has not ratified them): the average stand-in wins about 65 % on Easy, 50 % on Normal, 30 % on Hard, 15 % on Elite; the strong one clearly more at every tier.
- Stage 3: the chosen settings on all 57 conquest missions - a table per mission and tier (units, reinforcements, teammate tier for 1 v 0, 2 v 0, 4 v 0, 8 v 0, 2 v 1, 4 v 2), with the disc's own lopsidedness left in and the per-map spread shown.
- **Where it turns absurd:** the splits at which a lever passes a plain limit (reinforcements over 2x the mission's, enemy units over 1.5x, a teammate tier under Easy), and two or three proposed cap sets with what each costs in win rate.
- A page for the maintainer (`engine/artifacts/difficulty/README.md` + the tables) and a STATUS question. Nothing is built into the game here.

## Notes
Filed 2026-10-05 by Claude in chat from a design talk with the maintainer. His words are quoted; no number here is measured. Waits for BOTV: every number moves when the bots change.
