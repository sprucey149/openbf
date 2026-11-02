---
id: SPL11
title: The seams: think split by owner into behave / mover / gunner_foot with one MoveRequest and one AimRequest a tick - a pure move, the first build step of the bot design (was: brain.rs over the size cap)
milestone: M0
value: 5
phase: build
status: done
deps: OFFG,STLVL,STTAP,STARC,STEND,BOTD
files: engine/crates/core/src/ai/brain.rs,engine/crates/core/src/ai/behave.rs,engine/crates/core/src/ai/mover.rs,engine/crates/core/src/ai/gunner_foot.rs,engine/crates/core/src/ai/route.rs,engine/crates/core/src/ai/state.rs,scripts/size_waivers.txt,engine/CRATES.md
declares:
needs:
effort: high
branch: spl11
resume: landed 08e29070 2026-10-06
created: 2026-10-02
alpha: yes
---

## Why
Filed by cleanse CLNG (2026-10-02): `scripts/size_report.sh --strict` exited 1 on `core/src/ai/brain.rs` (2,032 lines, no waiver, no card). The waiver line in `scripts/size_waivers.txt` names this card. It depends on the `first:` AI cards because every one of them edits brain.rs and a pure move under them would cost each a rebase.

## Evidence

## Notes

## Planner 2026-10-02 17:45 - waits for BOTD
This card changes the soldier brain (`core/src/ai`) that the bot programme is about to redesign (the maintainer, 2026-10-02: "are we just implementing so many bandaid fixes to a fundamentally flawed bot algorithm"; BOTD: no rewrite before he has read the design). Held so no lane builds on the old brain meanwhile; BOTD adopts, rewrites or closes it when it cuts its build cards. Undo: remove BOTD from `deps:`.

## BOTD (2026-10-03) - this card is now THE SEAMS, build step 1 of the bot design
`analysis/bot-design.md` 3, 11, 12. Still a pure move (nothing declared, sim hashes identical), but along the design's seams rather than by size:
- `think` is split by owner into `ai/behave.rs` (what the bot is doing: today's priority chain as ONE function that returns one MoveRequest and one AimRequest per tick), `ai/mover.rs` (the walk block, arrival, the stance controller, the roll - the only place `pad.lx` / `pad.ly` / ROLL_STANCE / JUMP are written) and `ai/gunner_foot.rs` (the aim tracker, `seen_aim`, `fire_discipline`, `tap`, `burst_len`, `pause_ticks`, `slew`, `friend_in_line` - the only place the look sticks and FIRE are written); `advance_route` and `hub_reached` go to `ai/route.rs`.
- `AiState`'s fields are grouped by owner; none is dropped yet.
- No rule changes: the hold, the slide and the side-step stay as they are behind the new interfaces. Cards MOVR, OBST, FGHT and TPOS replace them one seam at a time.
Acceptance, added to the pure-move one above: the writers census of bot-design.md M7 reads ONE function assigning `pad.lx` / `pad.ly` and one assigning `pad.rx` / `pad.ry` / FIRE. If hash identity cannot be kept, say why, declare `sim; sweep; arena` and show a sweep inside the seed noise (bot-design.md 13, risk 1).
Waits for the maintainer's read of the note (deps BOTD). In BOTV's deps.
- 2026-10-05 planner - **The maintainer has read the bot design: "BOTD read, go" (INBOX 2026-10-03 14:08). All eight decisions take the note's defaults; the 16 deviations D1-D16 are all on. This card is released (BOTD is done).**
- 2026-10-05 planner: `phase` cleanup -> build so both lanes rank it with the other value-5 builds (a cleanup card sorts last in its value tier on both lanes; it would have waited six sessions). **The cleanup rule still holds: a pure move, every gate PASS, nothing declared.** `effort: high`. Undo: `loopctl.py set SPL11 phase=cleanup effort=`.

## Build 2026-10-06 - LANDED 08e2907 (5c39e91 the split, 08e2907 state / map / waiver)
`ai/brain.rs` is gone. `ai/behave.rs` (`think`, the chain; writes no stick), `ai/gunner_foot.rs` (`run`: the look
sticks and FIRE), `ai/mover.rs` (`run`: the move sticks, ROLL_STANCE, JUMP, the progress books), `advance_route` /
`hub_reached` in `ai/route.rs` (its tests now `ai/route/tests.rs`), `AiState`'s fields under an owner banner each.
Acceptance, each RUN (report `reports/20261006_1035_card_SPL11.md`):
- every gate PASS, nothing declared: ci 1318 / 0 / 43; sim walk e6fd27f2a5020f37 aim 38cfcc79bb2cba46 600
  47dbb5c32f091140, motion identical; simhash e20eb63fc9d796e6; sweep (32 maps x 60,000 ticks, a sim hash per row),
  arena flat / tat2 and the leak census byte-identical to the baseline.
- the writers census (`lead_tmp/spl11/writers_census.py`): move sticks 1 function (`mover::run`, 7 sites), look
  sticks 1 (`gunner_foot::run`, 2), FIRE 1 (`gunner_foot::run`, 3), ROLL_STANCE and JUMP `mover::run`; `behave.rs`
  assigns none. At f67c517 all 15 sites were in `think`.
- `size_report.sh --strict` exit 0 with the brain.rs waiver line deleted; `--check-map` 0 stale.
One thing the next cards should know: the gunner runs BETWEEN the AimRequest and the MoveRequest, because the hold's
strafe on a new burst, its step out of a friend's line and the close hold read the gunner's report (`GunReport`).
Card TPOS deletes those three rules; the MoveRequest can then be built before the gunner runs.
