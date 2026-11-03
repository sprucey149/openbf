---
id: TOKN
title: Hit tokens per human player by tier - a holder aims at him, the rest aim near misses; all fire stays real
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `done`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
Damage pacing (the user, 2026-10-28): each HUMAN player has a few attack tokens; a bot's shots at him can hit only
while it holds one, the rest still shoot and miss near him, visibly and audibly. Scaled so each tier is clearly
harder: Easy 1, Normal 2, Hard 4, Elite off (on the `tiers2` names). The bot design's "per-human pressure" layer
(`analysis/bot-design.md` 7: Half-Life's two attack slots per squad, Amalur's budget by difficulty).
`analysis/difficulty-proposal.md` proposes the same object as a per-human attack budget with Elite 6 and focus fire
at Hard / Elite (cards DFOCUS, DPRX). **Elite's value awaits the user's choice between off and 6**; it is one entry of
`ai::tokn::BUDGETS` (`tokens: None` is off).

## Evidence
No human plays in the sweep (slot 0 is a bot), so the measure is a human proxy built here: the arena's map field
with the idle player kept 1.5 m right of team 0's first living bot on foot, facing its way, at full health every tick
(`ARENA_HUMAN=shadow`; he stands in the fights, never fires back, never dies), one stderr `human` line per run.
10 maps (bf1 bes2 end1 geo1 kas2 nab1, bf2 cor1 dag1 kam1 myg1 yav1) x 4 seeds (42-45) x 9,000 ticks per tier,
`lead_tmp/botre/runs/TOKN/{before,after}_human.txt` (`before` = UNSN rebased on `tiers2` + the instrument):

| tier | damage taken / min before -> after | paired t | hits / min before -> after | paired t |
|---|---|---|---|---|
| Easy (1) | 435 -> 171 (-61 %) | -6.6 | 5.77 -> 2.48 | -6.9 |
| Normal (2) | 573 -> 221 (-62 %) | -5.4 | 8.74 -> 2.83 | -4.8 |
| Hard (4) | 1,256 -> 1,275 (+2 %) | +0.2 | 13.51 -> 12.41 (-8 %) | -0.8 |
| Elite (off) | 1,726 -> 1,726 | - | 14.40 -> 14.40 (every run's line identical) | - |

Hard is not easier beyond the noise (damage +2 %, t +0.2; hits -8 %, t -0.8), so Hard keeps 4.
Normal (the default) becomes about 60 % gentler on a human: that is the user's table as given, but it is a large
change to the default tier's feel and worth a play before it ships.

How many bots hit the human together (each hit on him by the distinct bots that hit him in the 3 s up to it;
`shooters_in_3s`), before TOKN - against a human, unlike bot-vs-bot (the proposal's 73-79 % from one shooter,
3+ at most 4 %), the pressure weight gangs the bots up:

| tier | 1 shooter | 2 | 3 | 4+ |
|---|---|---|---|---|
| Easy | 66 % | 27 % | 6 % | 2 % |
| Normal | 47 % | 29 % | 15 % | 8 % |
| Hard | 47 % | 33 % | 15 % | 6 % |
| Elite | 53 % | 30 % | 13 % | 4 % |

So a budget of 4 binds on at most the 4+ share (6 % of Hard's hits; tokens turning over inside 3 s lets 4+ still
appear after TOKN: 5 %); Normal's 2 binds on 23 % of hits and Easy's 1 on 35 %. After: Easy 94 / 6 / 0 / 0 %,
Normal 72 / 25 / 3 / 0 %, Hard 49 / 33 / 12 / 5 %.

## Change
`core::ai::tokn` (D24, OURS):
- `BUDGETS`: per tier `tokens` (Easy 1, Normal 2, Hard 4, Elite `None` = off), `hold` (5 / 4 / 3 s), `miss`
  (1.8 / 1.4 / 1.0 m). The tier is the attacking team's (`Sim::difficulty_of`).
- `deal` (once a tick, before the thinks, after the team board; `Sim::tokens`, hashed when not empty): a token
  whose holder no longer targets the human, lost its line, died, or whose human died is dropped; an expired one
  goes to the best-placed waiting bot, else its holder keeps it; free tokens go to the best placed. Best placed: an
  enemy bot on foot whose `ai.target` is the human, a clear `perception::los` from its muzzle to his centre, the
  smallest distance x (1 - 0.5 x how nearly its aim already points at him), lowest index on a tie.
- `gunner_foot::run`: a bot shooting at a human whose token it does not hold aims at `seen_aim`'s point plus
  `miss_offset` (the tier's `miss` to his right or left, the side flipping every 1.5 s by unit index and tick, and
  0.3 m up) and takes no grenade at him (the hand weapon instead); `NavStats::tokn_aimed_off` counts its firing ticks.
- The guarantee: `Sim::sweep_bolts` passes the human for such a bot's bolts, `resolve_bolt` does no damage for one
  the spawn ray aimed into him, and `blast_unit` spares him its blast (`NavStats::tokn_passed`). Only bots on foot are
  governed: vehicle and turret gunners, and humans' own fire, are untouched; bot-vs-bot is untouched.

How it combines with the rest of the human-facing layers (`analysis/bot-design.md` 7):
- The aim error model (the PS2's decaying tracked error by skill, `gunner_foot::seen_aim`, the tier's fire cone and
  first-shot delay) still decides whether a holder's shot hits; a token only permits it. A non-holder's aim point
  carries the same error plus the offset, so its misses wander like real shots.
- The D12 human-damage multiplier (x1.15 Hard, x1.3 Elite) is retired (card DRETIRE, the maintainer 2026-10-28: "Dying faster
  from a few bullets is a cheap way to do this"): every tier's hit on a human does the weapon's own damage. Per tier
  the danger to a human is: holders at once x their hit chance.
- The pressure weight (x1.3 / x1.6 on a human target) still decides who targets him, which is who can take a token.

## Deviation
D24 (`analysis/bot-design.md` 9): on the PS2 every bot's shot at a human can hit; ours only a token holder's.

## Update after the first play (2026-10-28, branch tokn2)
- **The user's new table:** Easy 2, Normal 3, Hard 5, Elite off.
- **No fire passes through a soldier any more** (the user: the game checks every bolt and blast). The sweep, the bolt
  impact and the blast no longer skip a human for a bot without a token. Such a bolt or blast that meets him anyway
  hurts him, and is counted: `nav_stats.tokn_stray`, and of those `tokn_cross`, aimed at someone else.
- **Grenades:** a bot without a token now throws them, but lands them short of him by the tier's miss plus
  `GRENADE_SHORT` (4 m) over the ground, instead of not throwing.
- **The near-miss side is fixed per bot** (by unit index), no longer swapped every 1.5 s. A bot that swapped sides
  swept its aim, and its shots, across him: in the unit test, 11 stray hits to the holders' 23 with the swap, at
  most 1 in 5 without (`in_play_the_token_holders_do_the_hurting` asserts that, so the swap is its mutation).
- **Arena human proxy** (same 10 maps x 4 seeds x 9,000 ticks, `lead_tmp/botre/runs/TOKN2/after_human2.txt`):

| tier | damage/min, no TOKN | first TOKN (pass-through) | now | hits/run | strays/run | of which crossfire |
|---|---|---|---|---|---|---|
| Easy | 437 | 172 | 442 | 31.5 | 14.65 | 13.00 |
| Normal | 579 | 222 | 446 | 27.6 | 11.07 | 10.28 |
| Hard | 1,263 | 1,285 | 1,310 | 64.2 | 4.40 | 2.42 |
| Elite | 1,741 | 1,741 | 1,741 | 69.0 | 0 | 0 |

- **Reading the table:** most of the first version's cut came from the pass-through, and most of that was crossfire.
  The proxy stands the human 1.5 m beside a fighting team-mate, so the fire aimed at the team-mate hits him; that is
  real now. Near misses aimed at him that hit anyway are about 1.6 a run at Easy (0.8 at Normal) against the
  holders' ~16. So pacing works on the fire aimed at him, but this proxy can't show its size. A proxy that keeps
  the human apart from the team-mates (or the user's play) is the measure. Hard is not easier (+4 %, noise).

## Open
- Elite: off or 6 (the user).
- The proxy is a passive human: a real player moves, takes cover and fires back, which lowers the shooter counts.
  Card DPRX's proxy (a bot treated as human) would give the fuller measure.
- A human standing in a bot's line of fire at someone else is passed by that bot's bolts unless it holds his token.
