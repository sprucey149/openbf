---
id: STEND
title: Rounds end once one side is clamped at alive = tickets: hunt the last survivors, and settle units crowding one spot (TAT2 11 v 1, BF1 GEO1 2 v 16, KAS2 3 v 2; BES2 stuck_max 9,060)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
STALL C7 + C9 (`analysis/re-round-stalls.md` §2).

**C7.**
- ObjectiveConquest stops the bleed at alive = tickets (re-tickets-spawn §1.3). Stock BF2 has no time limit and no cull
  (re-time-limit §0).
- So a clamped side's survivors must be killed. The port's commander offers Hunt only when no post is reachable:
  - TAT2 seed 8 ends 11 v 1: the Wookiee is walled off 12 m from cp3's two defenders, who never go after it.
  - BF1 GEO1 ends 2 v 16, KAS2 3 v 2.

**C9.** Units jostling on one point keep a stick deflected and never arrive:
- BES2's `stuck_max` reaches 9,060 from defenders on their spot;
- YAV1 seed 5 has 42 units of both teams on cp5's centre with no targets.

## Notes
- BF1's own stall exit is its exhaustion cull (900 s, then one AI per 15 s, re-tickets-spawn §2.6). The port does not
  run it, because PLAN plays every map under BF2's rules.
- If the maintainer wants BF1 maps on BF1 rules, that is a PLAN change.

## Note from the maintainer's "rounds that do not end" pass (2026-10-01, main 4603e9e, `lead_tmp/endings/README.md`) — `first: 7`, deps RCYC, STLVL, STTAP
- A first try at an endgame hunt rule in the commander (Fable, 2026-10-01, `lead_tmp/fable2/README.md` Task 3) cost Hoth,
  Endor and Tatooine rounds: the hunters ended up off the level (155 `lost` deaths on Hoth). Build it only once RCYC's
  census is there to show where hunters go wrong, and gate it on the 8-seed sweep.
- HOLD (4603e9e) already stops holding once a team is at 10 reinforcements or fewer (`HOLD_ENDGAME`).
- At 120,000 ticks the clamped endings left on main are BF1 YAV1 3 v 1 and 1 v 1 (STLVL's hub-17 spot) and BF2 KAM1
  16 v 8. Re-measure after the cards ahead of this one; if no round ends clamped at 3 alive or fewer, close this card's
  C7 half and keep C9.

## Note from RCYC (2026-10-01) - what the recycle does not end
RCYC (branch `rcyc`) recycles a bot that wants to walk and covers no new ground for 120 s once a team is at <= 10 reinforcements; rounds finished 253 -> 255 of 256 at 120,000 ticks. It never touches a bot that is where it wants to be, so two endgames are still this card's (`lead_tmp/rcyc/README.md`): (1) everybody defending - BF2 MUS1 seed 2 in variant c stood 3 v 3 with all six bots on their defence spots; (2) a firefight that kills nobody (BF2 KAM1 1 v 2, two rocketeers in sight; STTAP has the note). The gate's seed-42 sweep keeps BF2 UTA1 open at 60,000 ticks in the baseline (1 v 9, a bot pacing a 14 x 9 m box): with RCYC it ends.

- Planner 2026-10-01 22:00: the endgame hunt tried in BNDS lost Hoth / Endor / Tatooine rounds because the hunters were read as off the level (155 `lost` deaths on Hoth, `lead_tmp/fable2/README.md` Task 3). Card **LVLF** (value 5, measure) is the check of `Sim::level_at` on terrain; read its table before building a hunt rule, and carry `lost` deaths per map as a before / after line here.

## Note from LVLF (2026-10-02) - the '155 lost deaths on Hoth' of the dropped hunt rule
- Hoth's `lost` deaths were the level test, not the hunters: its trenches (2.0-2.5 m under the snow, in terrain cuts, between barriers) read off the level; 1,085 of BF1 Hoth's 4,067 trench cells, bots lost there 18 % of their time, 33 deaths per 8 rounds on main. Landed in LVLF: a trench is on the level, both Hoths 34 -> 0 lost deaths (`lead_tmp/lvlf/README.md`). The hunt rule can be tried again on Hoth.

## Note from STTAP (2026-10-02, branch `sttap`, `lead_tmp/sttap/tap`)
With the single-shot tap in, BF1 GEO1 seed 7 does not end: 9 v 2 alive at 60,000 ticks, tickets 9/2, stuck_max 3,420, longest
standoff 540 ticks — nobody is facing anybody, the survivors do not meet (the same seed ended 0/6 at tick 25,762 without the
tap). `SWEEP_SHOTS=1` gives a `standoff <map>:` line per map; one pair still stood 3,930 ticks on BF2 YAV1 seed 7 (units 9 and 28
from tick 8,430; unit 9 stands still with a target at (45.6, -40.2, -262.8)).
**Correction (land STTAP, 2026-10-02, main aecdc0e, `lead_tmp/sttap/land/repro_tap`):** the BF1 GEO1 round is seed **42**,
not seed 7 (seeds 1-8 all finish there). It is the gate's own sweep row now (`finished 31/32`), longest standoff 840 ticks;
without the tap the seed ended at tick 20,385. The BF2 YAV1 pair is seed 7 as written.

## Result (STEND build, 2026-10-02, branch `stend`, `lead_tmp/stend/README.md`)
Re-measured on main c9d2776 first (9 seeds x 32 maps x 60,000 ticks): 287 of 288 rounds finish; the one open round is
BF1 GEO1 seed 42 (9 v 2), and it is not a missing hunt: each last droid stands INSIDE a post platform's shell, under the
slab (left there by its spider walker), in the capture volume and 1 m from the centre in XZ, so it "had arrived".
- **Built:** a bot has arrived at its post or defence spot only on the spot's own floor (`brain.rs` `off_floor`,
  1.5 m; under the slab or on a roof over it). Its dwell and stall watch then run and the last resort gives it up.
- **Acceptance 1** (no round NO OUTCOME with the loser clamped at 3 alive or fewer): 288 / 288 finish (seeds 1-8:
  256 / 256); the gate's seed-42 sweep 31/32 -> 32/32.
- **Acceptance 2** (BES2 `stuck_max` max < 3,000): seeds 1-8 max 3,540 -> 2,700 (median 1,020 -> 540); seed 42 2,460 ->
  3,540. The tail is not a crowd (C9 measures 12 s at most): it is a last leg into cp1's terrace wall beside the
  stairs, noted on LLEG.
- **Acceptance 3** (the commander hunt test): NOT built, by this card's own rule ("if no round ends clamped at 3 alive
  or fewer, close this card's C7 half"). Measured instead (new `endgame` line of `SWEEP_DEATHS=1`): from the tick a
  side stands clamped at 3 alive or fewer the round lasts a median 24 s, p90 76 s, max 226 s; 1,059 of that side's
  1,107 last deaths are kills, 43 the last resort. Reverse: build the hunt if a later sweep shows endgames over 2 min
  on more than a few rounds (now 4 of 287).
