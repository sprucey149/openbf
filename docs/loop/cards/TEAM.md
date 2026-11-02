---
id: TEAM
title: Team sense: a unit grid for every 'who is near' question, roles with capacities inside a post's group (cover for a capturer, overwatch), fire lanes in the position choice, contacts passed on within 20 m (bot design 4.3)
milestone: M3
value: 5
phase: build
status: done
deps: BOTD,TPOS
files: engine/crates/core/src/ai/team.rs, engine/crates/core/src/ai/team/tests.rs, engine/crates/core/src/ai/behave.rs, engine/crates/core/src/ai/state.rs, engine/crates/core/src/ai.rs, engine/crates/core/src/perception.rs, engine/crates/core/src/sim/step.rs, engine/crates/core/src/sim/state.rs, engine/crates/core/src/ai/mover.rs, engine/crates/core/src/skill.rs, engine/crates/core/src/ai/gunner_foot.rs, engine/crates/core/src/ai/man/tests.rs, engine/crates/game/examples/round_to_outcome.rs
declares: sim; sweep; arena
needs:
effort: high
branch: team
resume: landed d4a50c62 2026-10-20; audit INCOMPLETE/UNAUDITED 2026-10-25: restore held inputs and verified history; see private note
created: 2026-10-03
alpha: yes
domain: ai
---

## Why
The maintainer, 2026-10-02: bots "all standing next to each other", shooting each other; "awareness of friends". `analysis/bot-design.md` 4.3 (the Team board), 4.1 (the unit grid). The PS2 has no team sense at all (SAIR 6). A teammate within 1.5 m: 8.7-17.9 % of a port bot's time against the PS2's 2.0-9.3 % (`lead_tmp/botm/post_jitr/seeds.md`).

## Notes
- Cut from `analysis/bot-design.md` (card BOTD, 2026-10-03). Waits for the maintainer's read (deps BOTD). In BOTV's deps.
- Not in it: a squad leader, formations, plans. Five roles at most.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).** Decision 4: **no garrison** (no Guard role that keeps 1-2 bots at a front-line post beyond the design's cover-for-a-capturer). Decision 5 (the flank share, 20 %, drawn 10-30 per match) is card FLNK's; the role's capacity here follows it.
- 2026-10-05 planner: value 4 -> 5: every card BOTV waits on is ranked with the chain, so the bot programme runs in the maintainer's order ahead of the fours (his notes of 2026-10-02 16:30 / 17:40 and 2026-10-03 14:08). Undo: `loopctl.py set TEAM value=4`. `effort: high` (the builds the maintainer's top concern rests on; a build runs at medium by default). Undo: `loopctl.py set TEAM effort=`.
- 2026-10-06 HUBP landed (408bc9c): what is left of "a teammate within 1.5 m" after the hub points, 8 seeds - KAS2 2.9, DEA1 11.6, KAM1 5.8, POL1 13.9 % (PS2 2.0 / 9.3 / 6.6 / 5.7). The remainder is bots standing at posts and holding (POL1: holders 6.6 % of unit-time, KAM1 cp2's idle bots) and POL1's in-file corridors; `lead_tmp/hubp/ab/near.py <dir> <map>` splits the close pairs by hold / still / move and by 6 m cell. Bar B3 is this card's.

- 2026-10-06 MOVR landed (6061eec): arrived bots stand (a latch), and "a teammate within 1.5 m" rose a little with
  it - 24 seeds: KAM1 6.5 -> 7.2 %, POL1 14.0 -> 14.8 %, DEA1 12.6 -> 12.3, KAS2 3.3 -> 3.2 (PS2 6.6 / 5.7 / 9.3 / 2.0).
  Bots that stand near each other no longer jitter apart; the reservation is this card's.
- 2026-10-07 planner, from SPRD's report (`reports/20261006_2233_card_SPRD.md`): **a gap in FFLN's line-of-fire check** - a salvo already queued when a friend walks into the cone still fires (the PS2's fire cycle fires a started salvo whatever the trigger does). One such shot in 900 ticks of `a_bot_does_not_fire_through_a_friend`, which now does not count it. Not carded (it did not hit); this card's fire lanes are where it belongs: count team hits from queued salvos in the `teamfire` line before deciding whether a started salvo should be cut.
- 2026-10-19 slice A (lane 1, branch `team` 194a57c; report `reports/20261019_1555_card_TEAM.md`). Re-measured on main
  7752d00 first (8 seeds, `lead_tmp/team/base`): B3 KAM1 3.3 / POL1 8.4 / DEA1 11.2 / KAS2 2.2 % (PS2 6.6 / 5.7 / 9.3 /
  2.0) - and the close pairs were mostly WALKING: same way, one behind the other, DEA1 5.9 of 14.3 % pooled, POL1 4.1 of
  9.0 (`lead_tmp/team/tools/files.py`, `near_split.py`). Built: the unit grid (`ai/team.rs`, hash-neutral: seeds 1-2 x
  32 maps identical) and the FOLLOW GAP (`team::follow_ease`: half stick a stride behind a mate walking ahead the same
  way): B3 KAM1 2.0, POL1 4.8, DEA1 7.0, KAS2 1.4 (8 maps x 8 seeds, `lead_tmp/team/varB`). Contacts passed on
  (`perception::tell`) BUILT AND HELD BACK (`Tier::tell` false on every tier): on, a told enemy keeps the bot in its fight
  behaviour at its fight spot (COR1 20 s spells 11.7 -> 24.4 %, "fight, a target, a hold" 5.5 -> 17.9 %;
  `tools/spells.py`); the first gate run refused it (idle20_pct, `lead_tmp/team/gates_run1`). Next slices: (1) a told
  contact WALKED toward, not stood at (behave's fight entry wants a seen target; then flip `tell` per the design's
  column, Easy off); (2) roles: Cover for a capturer (Hard and Elite only, the design's section 7) and Overwatch, with
  role counts per map and a capacity test; (3) the cost at 33 units (`lead_tmp/obst2/tools/perf.sh`, JOBS=1, a null twin
  of main); (4) move the Tactician's per-unit loops onto the grid (its reservations walk every unit).
- 2026-10-20 slice B LANDED d4a50c6 (lane 2; report `reports/20261020_0824_card_TEAM.md`): the telling ON (every tier but Easy) with
  `Contact::told` - aimed at, keeps nobody on a position, never overwrites the bot's own record (COR1 20 s spells 11.7 -> 8.5 %);
  fire lanes: every shot's line cleared by a tick of walking + its flight (`gunner_foot::shot_late`; the telling alone raised BF2
  team kills 307 -> 368); the team board's COVER role at Hard / Elite (`ai::team::board`, capacity 2, tests + 5 mutants); a
  circler with no ring point walks at an out-of-reach target (BF1 KAS2 Hard seed 2 NO OUTCOME on B1). B3 / B4 met, 256/256 at
  Normal and Hard, ms a tick x0.996 of main. Left (not carded): lanes held on the board (the Tactician asks each friend's line
  itself); Flank / Gunner roles are FLNK / TURR.
