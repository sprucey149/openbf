---
id: HSTD
title: Two stand-ins for a human in the player's slot, for the difficulty tests: an AVERAGE player (knows the game, plays sensibly) and a STRONG one (hundreds of hours: knows the maps, uses cover, vehicles, squad orders and the level design) - both human: no sight through walls, no knowledge of where the enemy is beyond what a player sees and hears, human aim and reaction; calibrated on cheat-free playtest logs
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
The maintainer, 2026-10-05: "You turn up the difficulty so the ai gets better so the user is more likely to die, how does that translate to making it harder to win games? If all ai is better including teammates, and the user just dies more, how does that make it harder to win a match? ... We need a good solution where victory is somewhat tied to user performance on any difficulty. Easier difficulties are a little more evenly split, while harder difficulties depend on the user playing well and thought needs to be incorporated for when multiple users are playing and they're on opposing teams." With equally good bots on both sides a battle is a coin flip the human tips; better bots only shrink the tip.

A bot in the player's slot plays nothing like a person, so the difficulty tests need a model of one. The maintainer: "an average human level player that understands the game and knows how to play and a strong human player that has played hundreds of hours of the game and knows the maps well, strategies, takes advantage of level design, teammates, etc while still being human and not being able to 'see' where all the enemies on the map are".

## Evidence
- **Do NOT calibrate on the playtest snapshots so far.** the maintainer, 2026-10-05: "during all my tests I've been turning on invincibility and infinite ammo ... those numbers are definitely wrong and shouldn't be trusted" (e.g. `playtest/last.json` of 2026-10-04: 62 kills, 0 deaths in 206 s). The black box does not record the active cheats today (BLKS has the note).
- `reports/20260909_1552_sweep-slot0-bot.md`: an earlier bot in slot 0.

## Acceptance
- Two stand-ins that drive the human's slot through the pad (the same inputs a player has), each with a written list of what it knows and does; perception is what a player's screen and ears give.
- Calibrated WITHOUT the maintainer's play (the maintainer, 2026-10-05: "Because the game isn't complete I wouldn't be able to get accurate results for you ... the gameplay just isn't right"), on three sources, each written down with its number:
  1. **The disc's own yardsticks.** BF2's award thresholds are the designers' statement of a notable life - READ THEM FROM THE ELF, both the normal and the higher-rank values (the web's list, unverified: Endurance 12 points in one life, Guardian 24, War Hero 36; Frenzy 12 blaster-rifle kills in one life, Gunslinger 6 pistol kills, Marksman 6 sniper headshots; lower at the higher award ranks; award ranks at about 4 / 31 / 64 earned, career ranks at 20 / 100 / 300 medals). And the auto-balance (`analysis/re-ai-difficulty.md`, the auto-balance paragraph: the human's team 25 tickets ahead moves the skill 1 step, 55 ahead the full 4): the designers' sizes for "winning comfortably" and "running away with it".
  2. **Published human numbers** (search summaries of 2026-10-05, to be opened and cited properly): a shooter player's see-to-shoot time about 500-600 ms; in DeepMind's Quake III CTF study humans tagged 559 ms after first sight with 48 % accuracy (arXiv 1807.01281); a skill study of a console shooter puts 1.55 kills a minute at the top 16 % and 2.0 at the top 2 % (arXiv 1411.1316 - check which game and mode); a forum player reports 331 kills / 3 deaths on Elite in the PC game (an upper anecdote of what a veteran does to the original bots).
  3. **A first proposal for the two profiles, PROVISIONAL:** AVERAGE - see-to-shoot about 550 ms, earns Endurance in some lives and Frenzy rarely on Normal against PS2-like bots, takes a post when one is near, seldom uses orders or vehicles. STRONG - about 350-400 ms, reaches the Frenzy threshold in most lives on Normal, plays the posts that decide the map, uses cover, vehicles and squad orders, knows the routes. Both: only what the screen and the speakers give.
- The stand-ins are tuned until their award rates and kills a minute against the PS2-parity bots at Normal sit where (1) and (2) put an average and a strong player; every table made with them says PROVISIONAL.
- When the game is playable for real, the maintainer's cheat-free rounds replace this calibration: BLKS's cheat flag in the snapshot header is what makes those rounds usable. No STATUS question to the maintainer before then.

## Notes
Filed 2026-10-05 by Claude in chat from a design talk with the maintainer. His words are quoted; no number here is measured. Best built on the redesigned bot's layers (BOTD chain), but it does not wait for BOTV: DIFH needs a first version.
