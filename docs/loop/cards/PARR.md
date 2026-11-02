---
id: PARR
title: A bot captures from inside the post's capture region, not on its centre: the last leg aims at a drawn standable point of the region (the PS2's rule) - BF1's posts have a solid plinth and BF2 Kamino cp0 sits 3 m up on the sample depot, so bots push on them (the maintainer's screenshot)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/route.rs
declares: sim; sweep; arena
needs:
effort:
branch: parr
resume: landed 6305dd94 2026-10-02
created: 2026-10-02
alpha: yes
---
## Why
The maintainer, 2026-10-02 (screenshot, card OBST): "This guy is just trying to run into this wall over and over. Is he trying to
stand directly on top of the cp or something?" Yes. OBST's measurement (`engine/artifacts/lead_tmp/obst/README.md`,
main 7b64fcd, 32 maps x 8 seeds x 10 min): a last leg at a post's own centre is 3,426 of the 16,224 stalled 10 s
windows (21 %). The port walks at `cp.def.pos` and arrives only inside the capture volume AND within 1.0 m of the
centre (`brain.rs` `arrived_at_post`, `CORNER_ARRIVE`).

## Evidence
- BF1's posts carry a solid plinth (`com_bldg_major_controlzone`, a cylinder about 2.6 m across): the closest approach
  is 1.5 m (median of 662 windows on BF1 KAM1 cp3, END1, YAV2, GEO1, YAV1, BES2, KAS2, RHN1, NAB1, KAS1).
- BF2 Kamino cp0 (the black-box room): the centre is 3.0 m over the floor on `kam_bldg_central_sample_depot ctf1`;
  189 windows, the map's top spot in 8 of 8 seeds; 84 % of walking time within 15 m of the post is blocked.
- BF2 Endor cp2's centre is inside `end_bldg_bunker_controlpanel` (a capsule at the centre is pushed 0.5 m).
- The PS2 (`analysis/re-conquest-goal.md` section 4, [V]): the capture behaviour walks to the post's position + a
  random point of its capture region; "at the post" is inside the capture region (`FUN_002a6d18`; without a region,
  within max(10, unit radius + owner radius + 5) m). It never aims at the centre.
- What-if (`lead_tmp/obst/arr2/`, `OBST_ARRIVE=2.0`): a 2.0 m radius takes the plinth windows 740 -> 43 and the depot
  189 -> 5, all stalls 5.39 % -> 4.74 %; rounds finished inside 10 min 195 -> 190 of 256 (seed 6: 25 -> 20). So not the
  radius: the PS2's rule.

## Result (build, 2026-10-02, branch `parr` 8f6ba88; A/B on one base: main 4c1460d + the observer vs the branch)
Main moved under the card: CONT (4c1460d) landed first and the 7b64fcd numbers above are not today's base (the
commander's 5 s rule takes a capturer off the post, so the long plinth / depot windows were mostly gone already).
Base re-measured, then the branch, 32 maps x 8 seeds x 10 min (`lead_tmp/parr/census_base`, `census_v2`):
- `post` windows on a post's own body (`com_*`): 44 -> 0 (last-leg `post` + `spot` on `com_*`: 56 -> 10).
- `post` windows 2,782 -> 1,994 - but the census calls a last leg `post` only when it aims at the centre, so part of
  that is a rename to `spot`: `post` + `spot` 6,861 -> 6,203. All stalled windows 6.46 % -> 6.11 %.
- New referee `lead_tmp/parr/pushing.py` / `pushatk.py` (0.5 s samples of a bot within 12 m of the post it attacks,
  on its last leg, stick pushed, under 1 m/s, no target): 1,355 -> 593; of them 1-2 m from the aim point (the ring
  round a plinth) 552 -> 25. BF1 all goals 1,001 -> 323, BF2 2,077 -> 1,792 (BF2's rest is defenders at hints, arc legs).
- BF2 Kamino's depot: 167 -> 130 windows, and 0 of them are last legs in either run: they are bots LEAVING cp0 on the
  arc leg hub 100 -> 103 with the depot between (OBST's class). The card's "189 -> under 10" is not met by this card
  and cannot be: note for OBST.
- `sweep_seeds.py` 8 seeds x 60,000 ticks (`seeds_base`, `seeds_v2`): 256/256 -> 256/256, every map 8/8 in both;
  mean finishing tick 13,428 -> 13,472 (+0.3 %); recycled 413 -> 378. Posts taken per round are not in the sweep's
  lines; the round length is the proxy.
- Without the contester rule (`seeds_v1`): TAT1 seed 3 did not finish (16 v 1 for 40,000 ticks at cp1, snapshot
  `lead_tmp/parr/bb/tat1_s3.json`) - hence the rule.
- Left (LLEG): a point behind a wall or on a lower floor (BF2 GEO1 cp1's bunker 144 push samples, TAT1 cp3's moisture
  farm 102, BF1 YAV1 cp5's bazaar 74).

## Notes
- LLEG is the rest of the last-leg class (a point on another floor, behind a doorway): build this first, then re-list
  LLEG's spots - some go with it.
- OBST (the wall-follow) edits `steer.rs` and `brain.rs` too: one at a time.

- Planner 2026-10-02 14:45: this is the maintainer's screenshot complaint and nothing may sit in front of it for a whole build: LLEG and WAI2 now wait for it (`deps`), OBST already did. `engine/crates/game/examples/round_to_outcome.rs` is taken OUT of `files:` because CONT (in progress on lane 1 since 14:24) lists it and would have kept this card ineligible until CONT lands. Use `lead_tmp/obst/tools/obst_census.patch` as a scratch observer; if you land an `OBST_CSV` switch in the example, rebase over CONT first (it adds `contact20` / `retarget` to the same file) and re-run the BASE census on the rebased main - CONT replaces the commander, so `lead_tmp/obst/all/census.txt` (main 7b64fcd) is not the base any more once it lands. Undo: `loopctl.py set PARR files=<the three paths>`.
