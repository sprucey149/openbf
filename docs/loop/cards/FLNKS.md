---
id: FLNKS
title: Flank share per attack: the board gives the Flank role to about 0.6 of the drawn share in battle (11.4 % of eligible attacks vs 20.1 % drawn; 82 % of its groups are under 3) - give the share per attack start, count every role end, and test the 20 rules the audit's mutations passed (AUDIT FLNK)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT FLNK (2026-10-20, PARTLY): the maintainer's decision 5 is "flankers 20 % per match, drawn between 10 and 30". In battle the team board gives the Flank role to about 0.6 of the drawn share. Of the attacks from a post with a flank route, 11.4 % get the role, against a mean drawn share of 20.1 % (obs_b1, 8 seeds x 32 maps; 11.9 % vs 20.2 % with 24 more seeds on five maps). Within the board's own groups, holders are 13.8 % of members against 20.1 % drawn, and every one of the 32 maps is below its drawn share (5 % on KAM1 / TAT2, at best 18.6 % on HOT1). The cause: the cap is round(share x group) over the group present at a 1 Hz board pass, and 82 % of group samples have fewer than 3 members (mean 1.75), so they get 0. Across maps, the share of small groups goes with the realized share at r -0.89. The acceptance test (2 of a static group of 10) passes on a group the battle almost never forms. Also: 35 of the 44 valid audit mutations of the card's rules pass every test; about 20 of them change battle behaviour.

## Evidence
- engine/artifacts/lead_tmp/audit-flnk/: tools/obs_patch.py (a read-only observer, null-proved: stdout byte-equal to the landing on 256 rounds; switch AUDIT_FLNK_MODE), tools/epi.py, sw/obs_b1, sw/obs_b2 (ep/ episode rows; the audit_flnk lines carry the once-a-second group tallies), wf/N5 (an agent's independent parser: 11.9 % vs 20.2 %, small-group r -0.893, cap rounding table), wf/C1 (observer audit).
- Variant T9 (group = every attacker of the post, whatever post its attack started from): 29.3 % realized. It overshoots because every route into a post counts the whole post's attackers, and B11 does not improve (pooled 16.32 % vs the landing's 15.96 %), so T9 is not the fix (sw/t9obs_b1).
- Stress: AUDIT_FLNK_MODE=all (every eligible first-leg member flanks) gives 88 % of eligible attacks the role; 21 flankers stalled (BES1 10, BF2 YAV1 5, GEO1 BF1 3 / BF2 2) against 1 at the landing's share (wf/C2). A higher real share must watch these legs.
- Surviving mutations with a battle effect (mut/, tools/mk_mut.py; judged in wf/MJ1-3): F1 (the +35 % bound at 1.6: the test's cases are 1.29 and 1.75), F2, F4, F11 (origin within 0 arcs: the build session's own 0-roles-on-Mygeeto case), F12, F13 (no join), F14, F15, T1 (ceil for round), T6, T7 / T8 (a coverer / sniper may flank), T9, T13 (the role kept when the leg is not a flank leg), P1, P3, P5, P7 (a new objective keeps the old route index), S4 (no flank at Hard / Elite), S5 (shares the same every match: "drawn per match" has no sim-level test).
- The sweep's `flank` line `ends A/D/T/O` misses 555 of 3596 roles (15.4 %): retarget_to_post wipes FlankState on a seat, a defend or a new post before the board sees it (wf/C1, wf/N6).
- Side notes: at Hard, Cover is given 7.9 % less often with Flank on (3132 vs 3399, paired t -1.92) and held 10.8 % less (t -2.32). The Cover pass does not skip a flank holder, so one unit can hold both roles (not seen in two snapshots, wf/C5). A unit stuck at its start hub counts as on its first leg and can take a flank slot while stuck (BF2 YAV1 s4 unit 1, bb/yav1_s4). bot-design.md 238's "a role is kept for at least 20 s" is not built for Flank (flank.since is never read).

## What to build
1. Give the share per ATTACK, not per standing group: e.g. a seeded draw per attack start (unit, life, pair) against the pair's share, or a running count per pair of attacks started and roles given that keeps given / started at the share. Keep "none in a group under 3" only if it still means something (a pair with fewer than 3 attackers alive), and say which in the doc.
2. Count every role end (seat, defend, new post) in nav_stats.flank_ends; skip flank holders in the Cover give (or let Cover end Flank, by rule).
3. Tests for the surviving mutations above (at least F1, F11, F13, T7, T13, P5, P7, S4, S5): each must fail under its mutant (tools/mk_mut.py has the exact substitutions).

## Acceptance (run and paste)
- Realized share within 2 points of the mean drawn share on the 32-map, 8-seed sweep (the observer's episode rows or an equivalent sweep counter), and on at least 28 of 32 maps within 5 points.
- B11 pooled no worse than the landing's 15.96 % (8 seeds x 32 maps), and recycled + stalled flankers no more than main's plus 8-seed noise.
- Every listed mutation fails a test (mut_run_nolock.sh log pasted).
- Sweep 256/256 at Normal and Hard.
