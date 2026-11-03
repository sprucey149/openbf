---
id: ENDH
title: Two last units hold in sight of each other and never kill: BF1 YAV2 seed 1 does not finish (tickets 1/1, both 'holding a position' 24 m apart, clear line, for 1000 s)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card WAI2's 8-seed sweep (main a582462, `lead_tmp/wai2/sw_v7`, null `sw_null` 256/256) left one round unfinished: bf1_yav2 seed 1,
NO OUTCOME at 60,000 ticks, tickets 1/1, alive 1/1, hash f1b0e56a10a2a5e5. Snapshot `lead_tmp/wai2/bb/bf1_yav2_s1.json` (tick 30,000):
unit 3 (imp_inf_pilotatst, (-4.3,-53.7,115.9)) and unit 19 (all_inf_vanguard, (8.4,-53.7,136.5)) are both on foot, 'holding a
position', steer idle, 0 m/s, each with the other as its seen enemy at 24 m, the line clear - and neither dies. No boarding claim on
either: not a vehicle state. Not looked at: whether either fires (ammo left? the vanguard's launcher out of rockets, the pilot's
pistol's range band?), or the hold keeps them out of a range they would fire from.
## Acceptance
1. Measure: `BLACKBOX_AT=30000 round_to_outcome 30001 1 bf1_yav2` + `blackbox_read --unit 3 / 19`, and a shots census over the last
   1000 ticks: name which rule keeps each from killing the other (no ammo, held fire, out of the weapon's band, misses).
2. Build what it shows; the check: bf1_yav2 seed 1 finishes, and 8 seeds x 32 maps beside a null lose no round.
alpha: a round that never ends is a player-visible defect when the last two meet like this.
## Planner 2026-10-02 23:55
- BF1 YAV2 is one of the six showcase maps, and JITR's landing left a watch on the same map (seed 42: stuck_max 10,080,
  a 1 v 16 endgame) - read that row too in step 1.
- No dep on BOTD: step 1 is a measurement. For step 2: a plain defect (no ammo left, a weapon band that excludes the
  range, a hold that never ends) is fixed here; if the answer is "the two teams need an endgame rule", it is a row of
  BOTD's design (the maintainer's annoyance 3: no bot idles as an end state; both teams hunt when few are left) - write the
  finding on BOTD's card and close this one without a brain patch. `files:` lists `ai/brain.rs`, which every held
  bot card also lists; they wait on the maintainer's read anyway.

## Planner 2026-10-03 03:35 - a second round of the same kind
HUBM's 24 extra seeds (report `reports/20261003_0045_card_HUBM.md`): **BF2 GEO1 seed 32 does not finish** on main
(3 v 3 alive at y -63, a survivor hidden on another floor; the pre-HUBM base finished 24 of 24). Audit HUBM: over 32
seeds both sides finish 1,023 of 1,024, so it is not a regression of that landing - it is a round that does not end.
Read it in step 1 beside YAV2 seed 1 (`BLACKBOX_ON=stuck round_to_outcome 60000 32 bf2_geo1`): if both are "the last
units never meet", that is one finding for BOTD's endgame row, not two patches.

- 2026-10-06 (found by LOC3; `lead_tmp/loc3/end/*.json`, binary `lead_tmp/loc3_ab_bin3`, a LOC3 variant that also
  located hunts by the PS2's locate - the stand-offs are older rules, the variant only drew new rounds): three more
  rounds that never end, all at tick 50,000 of 60,000. (1) BF1 RHN1 seed 27 (also seeds 13 and 21 of variant C; one
  in 32 seeds on main after LOC3, where cp7's hub moved 20 -> 40): the last two Imperials ride the AT-AT (vehicle 15,
  8,400 hp), stopped at its drop hub with the driver idle; 5-13 rebels 'hold a position' 14-25 m from it and never
  hurt it, it never fires at them. (2) BF1 NAB1 seed 30: 6 v 2 within 5 m of each other at hub 0, all sliding round
  `nab1_prop_ruins_column3` (body 14) at a leg point 1-4 m off, nobody firing. (3) BF2 HOT1 seed 28: 13 v 13, tickets
  13 / 13 - nine Imperials slide along `hoth_prop_trench_turrets` (body 288) at (-174, -9, 160) on a 127 m leg 3 -> 5,
  the rebels stand at their posts (defend).

- AUDIT OBST 2026-10-07: one more stand-off for the list - BF2 MUS1 seed 33 on e9ce6db does not end (tickets 14 / 1 at 60,000 ticks, stuck_max 20,880): the last defender stands inside `mus1_bldg_lava_control` at (-65.8, 66.6, 25.8) (`STOP Wall at 0.00 m`, stuck 546 s), the point of cp5, and 14 attackers circle within 4 m of him without a shot. Snapshot `lead_tmp/audit-obst/bb/mus1/bf2_mus1_s33.json` (tick 50,000). The parent's one unfinished round of the same 1,024 is BF1 HOT1 seed 32.
- 2026-10-07 (card MOVR2's session, a note): another stand-off of the same family, on the parent c0794eb and on
  main 6f6160b alike - **the losing side's last 3-8 units all sit in walkers nobody attacks**. BF1 Geonosis: 7-8
  clones in two AT-TEs (hp 14,300-14,985; one more in a smaller walker) parked 55-210 m from the 16 droids,
  who stand on their post with no target; tickets 7/84, bleed 0 / 0; NO OUTCOME at 60,000 ticks on 1 of 24 seeds on
  the parent (seed 10), 2 of 24 with the leg aim (seed 19; and BF2 Geonosis seed 18: 3 in an AT-TE against 1 in a
  walker, tickets 3/1), 3 of 24 with a give-up at the 3rd failed window (not on main). Snapshots at tick 50,000:
  `lead_tmp/movr2/bb/{base_b1geo_s10,leg_b1geo_s19,leg_b2geo_s18}/`. The 8-seed gate sweep (seeds 1-8) is 256 / 256.
- 2026-10-07 (card OBSR's session, a note; main d9db73c): one more unfinished round, drawn by the landing's changed
  rounds - BF1 Tatooine 1 seed 7 (tickets 1 / 1, alive 1 / 1 at 60,000 ticks; 256 / 256 on main e54841a over seeds
  1-8, 255 / 256 now; seeds 9-40 are 1,021 / 1,024 on both). At tick 50,000 the two last units are 370 m apart:
  #25 (team 1) stands still (speed 0) at (25.3, 9.7, 173.7), 0.9 m from its waypoint with `floor_dy` 2.05 and goal
  `attack cp0` (card STEND's kind: not on its post's floor), #7 (team 0) walks its legs at (-337, 23, 105) with goal
  `attack cp3`. Snapshot `lead_tmp/obsr/bb/final/tat1_s7.json`.
- 2026-10-20 AUDIT TURR (one more case, ENDH's own kind): BF1 NAB1 seed 14 on b19cdf9 (TURR's landing) is NO OUTCOME
  (tickets 1 / 1, alive 1 / 1 at 60,000 ticks; 767 / 768 rounds over seeds 1-24, the parent ce8975d 768 / 768 and today's
  main e95a687 finishes that seed at tick 26,906). At tick 59,990 the last two units stand 41.9 m apart, each with the
  other as its target, `doing fight`, `hold 2`, `fmode 3`, speed 0, full health (300), and no event in the last 30 s
  (#5 team 0 at (93.7, -110.0, 79.3), #17 team 1 at (119.7, -114.3, 46.7)); neither is in a turret or claims one.
  Snapshot `lead_tmp/audit-turr/runs/nab14/bb_land/bf1_nab1_s14.json` (BLACKBOX_AT=59990).

## Cloud preflight 2026-10-24 07:40 UTC
Blocked before measurement on main 3d487d9: engine/assets_baked/, engine/missions/, cited wai2/JITR and later stand-off snapshots/raw runs are absent; iso/ and extracted/ are absent too. No map replay, unit-3/19 digest or last-1000-tick shot census ran. JITR seed-42 watch read from its historical report only. Cause remains unmeasured; no AI patch or BOTD classification. Restore inputs and reproduce on current main before changing behaviour. Report: `reports/20261024_0740_card_ENDH.md`.
