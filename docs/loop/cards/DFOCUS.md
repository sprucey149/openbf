---
id: DFOCUS
title: Focus fire on human players at Hard and Elite - a human's token holders see him, take him as their target and converge; Elite's tokens capped at 7
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
The user, after the first play of TOKN (2026-10-28): Hard and Elite are too easy overall. Elite gets a token cap,
7, a little above Hard's 5, so the whole map never converges on a human. At Hard and Elite the bots that can
perceive a human deliberately make him their target and converge on him (focus fire), up to the tier's token count:
the token holders are the focusers. Easy and Normal: no focus. `analysis/difficulty-proposal.md` (2, 6 row 5)
proposed the same: today a bot shoots the nearest contact it perceives, weighted for a human by the tier's pressure.

## Change
`core::ai::dfocus` (D25, OURS) and `ai::tokn`:
- TOKN table: Elite `Some(7)`, hold 2 s, miss 0.7 m (Hard 5, 3 s, 1.0 m), PROVISIONAL.
- The deal (`tokn::deal`, once a tick before the thinks) at Hard / Elite also offers a human's tokens to a bot that
  does not target him yet when `dfocus::may_focus`: its own eyes saw him at its last think (a heard shot within
  30 m or a friend's word makes a record and turns it, no token); he is out of his 3 s spawn grace
  (`Sim::focus_born`, hashed); within its reach (`dfocus::reach`: 45 m on foot, 120 m with a scoped rifle, 90 m
  against a human in a vehicle or turret with a weapon that hurts that hull, never without one; a bot with only
  heavy weapons does not focus a soldier); it is not capturing (in its objective post's region, the post not its
  team's) nor defending a post its team saw threatened in the last 5 s; a clear line from its muzzle. A bot holding another human's token is offered his only if it targets him.
  The best placed first, as TOKN's.
- A holder keeps the token while it sees him, or, after losing sight, for `MEMORY` (3 s) while its own record of him
  is that young and his last seen point is in reach (`dfocus::remembers`); then it lets go.
- The think (`behave::brain`): after the target choice a holder at Hard / Elite takes the human whose token it holds
  (`dfocus::pick`: seen first, then nearest) as its target (`perception::take_target`, past the 1.5 s interval
  and the hysteresis). One that lost sight walks to his last seen point when it is within 25 m (`dfocus::push`),
  unless it holds a Tactician position (which already fights a remembered target, card TPOSR).
- Spread: the Tactician's spaced try counts a position within 30 degrees of a fellow focuser's bearing from the
  human as crowded (`dfocus::fellows`, `tactic::choose`), then falls back as before.
- Line of fire blocked by a team-mate: card FFLN's rule stands for focusers (no shot, the holder steps aside).
- Bots in seats never focus (tokens govern foot bots). Bot-vs-bot and Easy / Normal are untouched.

- Fix (second commit, every tier): TOKN's line to the human passes his own vehicle's hull and his turret's
  building (`perception::los_seat` with `past_mounts` / `past_buildings`, one test for all tiers). Before, a human in
  a vehicle or turret never got a token holder - his hull blocked the line - so at Easy / Normal every governed bot
  aimed beside him: immune to aimed fire. Test `a_human_in_a_vehicle_gets_token_holders` (Easy, a launcher bot, a
  hull round the pilot); its mutation (every hull blocks again) fails it.

## Deviation
D25 (`analysis/bot-design.md` 9): on the PS2 nothing gangs bots up on one human; ours focus at Hard and Elite.

## Evidence
Two human proxies in the arena (map field, 10 maps x 4 seeds 42-45 x 9,000 ticks, every tier;
`lead_tmp/botre/runs/DFOCUS/{before,after}_{shadow,lone}.txt`; `before` = tokn2 18789c1d + the new proxy, its shadow
rows identical to TOKN2's `after_human2.txt`): `ARENA_HUMAN=shadow` (TOKN's, 1.5 m beside a team-mate) and the new
`ARENA_HUMAN=lone` (15 m behind team 0's front bot, on a standable point at least 6 m from any team-mate, in sight of
the nearest enemy where one is, chosen again every second). Damage per minute alive (mean of runs, paired t):

| tier | shadow before -> after | lone before -> after | 2+ shooters in 3 s, shadow | lone |
|---|---|---|---|---|
| Easy | 442 -> 442 (40 of 40 runs identical) | 476 -> 476 (identical) | 33 -> 33 % | 39 -> 39 % |
| Normal | 446 -> 446 (identical) | 535 -> 535 (identical) | 35 -> 35 % | 41 -> 41 % |
| Hard | 1,310 -> 1,525 (+16 %, t +1.9) | 1,743 -> 2,025 (+16 %, t +1.8) | 55 -> 57 % | 54 -> 58 % |
| Elite | 1,741 -> 1,768 (+2 %, t +0.2) | 2,133 -> 2,553 (+20 %, t +2.5) | 47 -> 51 % | 52 -> 61 % |

Shooter histograms (1 / 2 / 3 / 4+): Hard shadow 45/32/16/7 -> 43/31/16/9, lone 46/31/16/8 -> 42/31/15/13;
Elite shadow 53/30/13/4 -> 49/30/12/9, lone 48/30/14/8 -> 39/27/17/17. Hits a run: Hard 64 -> 79 (shadow),
100 -> 113 (lone); Elite 69 -> 70, 107 -> 129. Before, Elite was TOKN-off: with the cap of 7 and focus it is not easier
than that in either proxy, but in the shadow proxy it is only level (+2 %, noise); the lone proxy shows the gain.

## Open
- Starting values to tune by play: reaches 45 / 120 / 90 m, memory 3 s, push 25 m, grace 3 s, spread 30 degrees,
  Elite hold 2 s and miss 0.7 m.
- The proxies never move, take cover or fire back; the user's play is the real measure.
- Census sweep (round_to_outcome) not rerun for this card.
