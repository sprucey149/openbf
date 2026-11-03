---
id: LOC3
title: Locate as the PS2 does: 3-D tests (BF1's hub is a sphere, both games' corridors and last resort are 3-D, only BF2's hub disc is XZ), the corridor with the smallest rA + rB, and a goal arc kept as an arc with the end the route reaches - the port's XZ locate with a fixed nearer end puts 849 of 921 live PS2 soldier goals on the PS2's hub or arc (629 on its destination hub), and the port's own level rule moves a bot's start hub away from the PS2's (BF1 KAS2 86 %, BF2 POL1 80 % agree)
status: archived
---

Historical technical record. Recorded state before archival was `done`.

## Why
Where a bot's route starts and which hub a post or a goal resolves to both come from `NavGraph::locate_on`. Audit
HUBM (2026-10-03, OK) re-read the PS2's locate from both ELFs' instructions and held a transcription against every
live path object in the save-state library: it reproduces the PS2's stored answer for 1,004 of 1,004 distinct goals;
the port's reading - everything in XZ - reproduces 726. What the port does differently, each measured:
- **XZ instead of 3-D.** BF1's hub test is a sphere and both games' corridor and nearest-arc tests are 3-D; only
  BF2's hub disc is XZ (RE_LOG "Arrival and locate"). The port's doc comments say the opposite (`nav.rs`
  `locate`: "in XZ"; `ai/route.rs` `level_hub`: "the PS2 locates in XZ alone (RE_LOG ...)").
- **A fixed nearer end.** For a goal off every hub the PS2 keeps the ARC and takes the end the route draw reaches
  (else the nearer end); its stored destination is the farther end in 39 % of live arc goals. The port resolves a
  post once, to the nearer end. For a start on an arc the PS2 routes by the draw, else from the farther end.
- **The first containing corridor** instead of the one with the smallest rA + rB (8 of 1,170 live goals differ).
- **BF2's arc flags:** the corridor step skips arcs with flags & 7, the last resort arcs with flag bit 0; the port
  skips only jet leaps (bit 2) for a jetless mover.
- **The port's own level rule** (`hub_on_level` preference in `locate_on`, `level_hub`, card STLVL - "OURS,
  PROVISIONAL") was built on the XZ premise. At PS2 soldier positions it moves the start hub AWAY from the PS2's far
  more often than toward it: BF1 KAS2 1,499 away / 47 toward, BF2 POL1 3,748 / 226.
Net today (main ceb269b): the port's hub is the PS2's hub or an end of its arc at 849 of 921 live soldier goals
(its destination hub at 629); 183 of 185 posts (not BF1 BES1 cp15, RHN1 cp7); the in-game start hub at 99.2 % (TAT1),
96.0 % (NAB1), 93.5 % (BF2 KAM1), 87.5 % (DEA1), 86.2 % (KAS2), 80.0 % (POL1) of soldier positions. Showcase YAV2:
11 of 15 live goals. The bot design (`analysis/bot-design.md` 4.5) keeps the Navigator and does not name these.
What it costs in play is NOT measured - that is this card's referee.

## Evidence
`engine/artifacts/lead_tmp/audit-hubm/README.md` (the table, every tool). The fixture: `ram/path_fixture.json`
(1,170 live path objects, 1,004 distinct: game, state, filter, goal, the PS2's result 0 / 1 / 2, destination hub, the
goal arc's other end). The model: `tools/ps2loc.py` (BF1 `FUN_00266678` / `FUN_00265d38` / `FUN_00265d90` /
`FUN_00266840` / `FUN_00265eb8`; BF2 `FUN_002eb918` / `FUN_002ea878` / `FUN_002ea908` / `FUN_002ebb48` /
`FUN_002eaae8`; the end choice in `FUN_00266e50` / `FUN_002eca30`). `tools/variants.py` scores a reading against the
fixture; `goals_vs_ram.py`, `posts_vs_ps2.py`, `start_vs_ps2.py` score the port (they read the `AUDIT_NAV` dump of
`observer_af03b6c.patch`). The baked graph equals the PS2's RAM graph on 32 of 32 maps (`tools/goals_pts.py`).
RE_LOG: the planning-graph entry's "Audit 2026-10-03" addendum.

## Notes
- Behind BOTD like every bot build card (the maintainer's note of 2026-10-03 00:20): it moves every bot's path.
- The end choice needs a route that ends on an ARC (the PS2's path keeps `[3]` the end it reaches and `[4]` the
  other end, and finishes at either). A smaller first step that keeps one hub per post: measure in 3-D and take the
  smallest corridor - that alone moves the 921-goal score from 849 toward the loose ceiling.
- `Locus::Nearest`'s doc line still says "the nearest hub in XZ" (since HUBM it is the nearer end of the nearest arc).
- BF2 Mustafar: 2 hubs whose only soldier arcs are jet leaps are skipped by the port's mask and not by the PS2's
  (declared OURS in `locate_on`).

## Planner 2026-10-03 03:35 - AQD merged in; this card gates BOTV
- **AQD (2026-09-25, value 2, post-alpha, no acceptance) is closed as merged into this card.** It was the same defect
  seen from one spot: BF1 YAV1's aqueduct, STK2's trace `lead_tmp/stk2-census/yav1_u8.err` (seed 42): team-0 unit 8 on
  arc 110 (hub 4 -> 83) re-plans at T10380 at (57.2, -61.5, 52.8), 17.5 m from hub 83; `hub_for_on` answers 83 (the
  nearer end), the new leg walks straight at hub 84 and leaves the aqueduct's deck. With "a start on an arc routes by
  the draw, else from the farther end" that re-plan stays on the arc. Add it to the acceptance as one test: the
  re-plan at that position keeps the unit on arc 110 (or the PS2's stored answer for it, if a library state has one).
- This card is now in BOTV's deps (it moves every bot's path), so PERF and UCNT wait for it too.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set LOC3 value=4`. `deps` gains SPL11 (it shares `ai/route.rs` with the seams, and the maintainer's order is SPL11 first). Undo: `set LOC3 deps=BOTD`.
- 2026-10-06 (found by HUBP): BF2 KAM1 hub 92 (82.4, 149.2, the pod room) resolves its floor to 50.1 under a deck at 52.85, so arc 101 (92 -> 93) is marked unwalkable from 92 and every plan from 92 to dest 86 falls to a straight line 228 m long (`nt.straight`); a bot that first sticks at that doorway loops 91 -> 92 -> straight -> stuck. Seen for 12,060 ticks in an HUBP prototype (seed 5, unit 32); base enters it more rarely. Pre-existing; check when the 3-D locate lands.

## Landed 2026-10-06 (f5990f8) - what did and did NOT land (report `reports/20261006_1355_card_LOC3.md`)
- LANDED: `NavGraph::locate` = the PS2's (fixture test 1,004 of 1,004, mutation-checked; posts 185 of 185).
- NOT landed, by acceptance 3's measurement: the PS2's locate for UNITS, the start-on-an-arc rule, the goal arc kept
  as an arc, the level rule's removal. They are named deviation D17 (`analysis/bot-design.md`) and card LOC3B.
  32 seeds x 32 maps: the PS2 everywhere 1,315 recycles against 838, finishes 1,022, 12 maps worse by |t| >= 3.
- AQD (merged in above) is therefore NOT fixed: its test passes with the start rule and waits in
  `lead_tmp/loc3/locate_ps2_with_aqd.rs` for LOC3B.
- The 2026-10-06 HUBP note (BF2 KAM1 hub 92's floor under the deck) was not looked at: the unit locate did not change.
