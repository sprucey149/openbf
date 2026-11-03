---
id: HTAU
title: BF1 Hoth's far tauntauns: two pads sit beside a trench the claimer cannot cross (2-12 claims given up a round) and the PS2 starts 5 rebels near the far pads where the port starts none - 6 of 8 tauntauns ridden at 30 s against the PS2's 7
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card WAI2 (a582462, 2026-10-02) left three BF1 Hoth items as notes on a closed card (its 'NOT met / open' 2, 3 and 5);
this card is their record so they are not lost. BF1 HOT1 is not one of the six showcase maps: `defer: post-alpha`.
1. **Claims given up at two pads.** BF1 HOT1 still gives up 2-12 vehicle claims a round, nearly all on tauntauns at the
   pads (-304.6, -36.6) and (-412, 172.8): the claimer stands in the trench beside the pad - same hub, a wall between.
2. **Missing rebel starts.** The other two tauntauns unridden at 30 s stand 62-200 m from any bot. The PS2 starts 3
   rebels at (-450..-425, 175) and 2 at (-500, -75) (`lead_tmp/wai2/ps2/b1hot1_sold.csv`); the port starts none there.
   Which spawn path those PS2 units come from is not read (a post's spawn path? CWSP built the command walkers' points).
3. BF2 HOT1's second AT-AT pilot is seated after 30 s (PS2 24 s) - reported only, no bar.

## Evidence
`reports/20261002_2250_card_WAI2.md`; WAI2's card, section 'Built and landed'; the walkers line of
`ROUND_VERBOSE=1 round_to_outcome` at 30 / 60 / 120 / 180 s and the `vehicles:` line (`ai_claims / given up / lost`);
the scratch per-reason census `lead_tmp/wai2/v6_with_census.patch` (on 142dd4b).

## Acceptance
- Measure first (4 seeds 42 / 1 / 2 / 3, as WAI2): where each given-up claim's claimer stood, and the port's unit
  positions in the first 10 s beside the PS2's csv; name the spawn path the PS2's five rebels use.
- Then: BF1 HOT1 tauntauns ridden at 30 s >= 7 of 8 on each of the 4 seeds (today 6 of 8; PS2 7 of 8); claims given up
  <= 1 a round (today 2-12); units inside 30 m of (-437, 175) and of (-500, -75) in the first 10 s >= 3 and >= 2;
  8 seeds x 32 maps beside a null lose no round.

## Notes
- Filed by the planner 2026-10-02 23:55 from WAI2's leftovers (one card, not three). `files:` is left empty for the
  measure pass; the build will touch `core/src/ai/ride.rs` / `route.rs` (VDROP's files) or the spawn pick - set them then.
  Undo: close it as a note on VDROP.
