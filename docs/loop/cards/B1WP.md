---
id: B1WP
title: BF1 Geonosis / Hoth: a live command walker is a held post (BF1's rule, re-command-walker-post 0.6 [O]) - the port's BF2 all-posts countdown ends all 16 rounds the moment the attacker's ONE fixed post falls (some at 77 s on 197/235 tickets); team 1 wins 16-0 on both
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card WINS (2026-10-22): BF1 Geonosis and BF1 Hoth go to team 1 in 16 of 16 seeds, and EVERY one of WINS's 16 re-run rounds
ends by the port's 20 s all-posts countdown (`cp::tick_domination`), some at 77-200 s with both sides on 150-235 tickets
(`lead_tmp/wins/run1/bq/bf1_{geo1,hot1}_s*.ev.csv`, the `cp` rows: every post team 1's, 20 s, then the end). The attacker
(BF1 GEO1 Republic, BF1 HOT1 Empire) starts with ONE fixed post (GEO1 `1110`, HOT1 `11011`) and its AT-TEs / AT-ATs.
On the PS2 BF1 a live command walker's post counts: `analysis/re-command-walker-post.md` section 0.6 / 7 [O] - the BF1 team
update `FUN_002ad830` sums ValueBleed over every scheduled post the team owns (the walker's 10) and the 20 s "no posts"
countdown runs only at value 0, so a live AT-AT holds it off. The port plays BF2's rule on every map (PLAN, Modes & rules,
"Conquest: BF2 rules"), and BF2's own rule leaves the walker out - which is fine on BF2's maps (they register more fixed
posts) and ends every BF1 walker battle the moment the one fixed post falls.

## The maintainer's decision (2026-10-22; closes STATUS Q70)
Yes: use BF1's command-walker post rule on BF1 Geonosis and Hoth. A live walker is an
uncapturable held post, ValueBleed 10; a destroyed walker contributes zero until respawn.
The maintainer recalls respawn at the original pad and the crew boarding and advancing again;
Geonosis advances into enemy space, Hoth attacks the shield generator. The generator is
an uncapturable post until destroyed, then removed. These details are a requested design
and memory to VERIFY against the PS2, not fresh PS2 findings. Name any disagreement in STATUS.
The PLAN and FEATURE_SET entries now record the approved exception. No further yes is needed.

## Research before build
Re-phase to re: existing re-command-walker-post.md is the starting evidence, but does not
by itself prove every newly requested lifecycle/objective detail. Check both mission scripts,
walker/generator class and placed objects, live PS2 ownership/countdown/bleed rows, death,
respawn and crew goal traces. Follow the RE checks and independent verifier discipline.
Then scope build ownership around the actual setup, post, vehicle-pad and AI paths.

## Acceptance
- A sim test: a team owning no fixed post but a live command walker is not defeated after 20 s (BF1 mission); it is after
  the walker dies (mutation-checked).
- 16 seeds: no BF1 GEO1 / HOT1 round ends by domination while the loser has a live walker; report the win share (main
  0-16 both).
- Reference table for each map: live walker, all walkers destroyed, respawning, reboarded,
  generator alive/destroyed; actual PS2 vs port held/uncapturable status, bleed value,
  countdown and objective. Record exact PS2 differences from the maintainer's recollection.
- Mutation-checked lifecycle scene: fixed posts lost with a live walker survives beyond
  20 s; death removes its 10; respawn at the original pad restores it; crew advances again;
  Hoth generator cannot be captured and disappears as a post at destruction, by the measured rule.
- Retain the 16-seed no-live-walker-domination bar. Report both win shares; no side over
  12/16 unless PS2 bots-only trials establish the same lean, and trace every unfinished round.
