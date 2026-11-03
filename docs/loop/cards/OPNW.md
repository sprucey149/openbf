---
id: OPNW
title: The opening wave draws a post, then a node, as the PS2's members do - today the bots are dealt round-robin over the flat list of every post's nodes (and the pads' command walkers' points), so a post with more nodes starts with more bots
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card BSPN made a bot's RESPAWN draw one eligible post and then one of its nodes (the maintainer, 2026-10-07: 'make the bot spawn the same as the ps2'). The opening wave was left as it is: `WalkSim::opening_wave` (`game/src/walk.rs`) deals `ai_per_team` bots round-robin over `Sim::spawn_points` plus `pad_walker_spawn_points`.

Planner 2026-10-24 08:25 UTC: OPNW remains the sole opening-wave implementation
owner. SPST is now a blocked measure follow-up depending on BSPN and OPNW; it
retains the exact respawn referee, opening-walker shares, 240-second spawn and
all-map outcome checks, plus the HOT1 walker-light diagnostic. Preserve per-walker
opening columns in OPNW's census so SPST can check its bars without duplicating
the implementation. This division changes no OPNW acceptance or restoration hold.

## Evidence
- The PS2's opening members draw a post by the same rule (re-command-walker-post 6, RAM: BF1 Hoth's 24 Empire AI chose the fixed post 10, the AT-ATs 9 and 5; BF2 Hoth 18 AI: 5 / 5 / 4 / 4).
- The port's opening column: `SPAWN_CENSUS=1` `spawns <key>:` line, `opening` (runs in `lead_tmp/bspn/sweep_fin`).
- `Sim::draw_spawn_post` / `spawn_post_nodes` (BSPN) do the draw for live posts; a pad's not-yet-built command walker needs a post of its own in the opening (`pad_walker_spawn_points` returns a flat list).
- Risks to check first: tests that pin the round-robin places, map goldens that draw the opening soldiers, several bots on one node at tick 0 (the PS2 has no 'occupied' test, and spreads the opening over about a second).

## Acceptance
- A test: posts of 1, 2 and 6 nodes each start with a third of the opening bots (within the draw's noise over many seeds).
- The `spawns` line's opening column over 8 seeds x 32 maps, before and after, beside an even share per post; the sweep's finish, stuck and recycle counts before and after.

- 2026-10-09 12:45 planner. `deps: BSPN`: the same rule and the same file (`game/src/walk.rs` is in BSPN's `files:`), and BSPN's branch is built and waiting on the maintainer; built first, this card would collide with it. Undo: `loopctl.py set OPNW deps=`.

- 2026-10-12 15:21 AUDIT BSPN (`loop/audits.md`). Size of what this card owns: the opening wave is 13.5 % of a round's spawns (8,448 of 62,753 over 8 seeds x 32 maps on the landing 2d1fb60; 13.4 % on main d096ae7; `spawns` rows, opening against round), all dealt round-robin over the flat node list. The PS2 side of the rule is now on firmer ground than Hoth's three counts: six independent PS2 captures (737 AI life starts on BF2 Dagobah, BF2 Kamino, BF1 Bespin) reject the flat list (chi2 54.5, 8 dof) and fit the even post draw (9.8) - `lead_tmp/audit-bspn/wf/K4/out3.txt`; those captures skip each slot's first appearance, so the OPENING members' own draw is still the RE's reading (re-command-walker-post 6) plus Hoth's 10 / 9 / 5. `deps: BSPN` is met (landed 2d1fb60). Card SPTH touches the same file (`walk.rs`) for a test of the weight's plumbing.

- 2026-10-22 04:00 WINS2. A scratch prototype of this card (`game/src/walk.rs` `opening_wave`: with no pad walker, each bot
  `draw_spawn_post` then that post's next node; worktree `openbf-wt/wins2-opnw-scratch`, target
  `lead_tmp/wins2_scratch_target`) on BF2 Mygeeto: the Alliance opened 4 at cp1_con / 13 at cp7_con (main: 17 / 0, the
  flat list deals all 17 to cp1_con's 16+ nodes), and the win share stayed 3-13 over 16 seeds (`lead_tmp/wins2/ab/`). The
  rule is still the PS2's; it is not MYG1's one-sidedness (card MYGC).

- MYGC (2026-10-22): value 3 -> 5. The PS2 opens bf2_myg1 at 10 / 6 (Alliance forward / home) and 9 / 7 (Empire) in 3 of 3
  rounds sampled from the level load (`lead_tmp/mygc/ps2/bf2_myg1_o{1,2,3}_*`); the port opens 0 / 17 and 6 / 10. With
  AITHK's level-0 think it takes showcase Mygeeto from 3-13 to 9-7 (card MYGC's table); alone it does not move it.
