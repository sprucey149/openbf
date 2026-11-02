---
id: SPRD
title: Sustained fire stays tight 1-2 s longer before the spread opens, for the player and the bots (the maintainer, 2026-10-04; a NAMED DEVIATION - the port's spread rule is the PS2's)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/weapon.rs
declares: sim; sweep; arena
needs:
effort:
branch: sprd
resume: landed eece09f0 2026-10-06
created: 2026-10-04
alpha: yes
---

## Why
The maintainer, 2026-10-04: "Have bullet tight spacing stay tight for like another second or 2 before having it start increasing the spread wildly, like when a bot or user holds the fire button and the bullet spacing is tight then begins to spread more, have it stay tight a little while longer."

## Evidence
- The port's spread is the PS2's rule (`core/src/weapon.rs`: `spread_per_shot`, `spread_threshold`, `spread_limit`; a human starts at -SpreadThreshold). So this is ours, on purpose.

## Notes
Filed 2026-10-04 by Claude in chat from the maintainer's playtest (build 1bde70c, main 4e51e84). The maintainer's own words are quoted; nothing here is measured unless it says so.
- 2026-10-06 planner: value 4 -> 5. Why: it is the maintainer's own playtest ask of 2026-10-04 and still open after 23 sessions; of that note's cards HUDW, ANIM, ANNC2, ERAM, ZOOM (RE) and WADE (RE) have run, this one and GRNP had not been reached (at value 4 the M3 fours go by ID and this stood 13th). Two things for the builder: (1) `declares:` is empty - fill it (`sweep; arena; sim` as measured) before the gates; (2) the bots take the deviation too, so paste the sweep's kills / min per map before and after: BOTM's kill-rate bar is read against the PS2's column (card BOTV), and a tighter bot burst moves it. Undo: `loopctl.py set SPRD value=4`.
- 2026-10-06 22:33 build, LANDED eece09f (report `reports/20261006_2233_card_SPRD.md`): `TIGHT_HOLD_SECS` / `TIGHT_HOLD_SECS_AI` = 1.5 s in `core/src/weapon.rs` (a buffer under the PS2's floor, capped at SpreadLimit and 6 deg; 0.0 = the PS2's). Acceptance run: test `held_fire_stays_tight_one_to_two_seconds_longer` (fails with the constants at 0); per-class table `lead_tmp/sprd/sprd_table.txt` (BF1 rifle 2.29 -> 3.79 s, BF2 rifle 1.33 -> 2.83 s, bots 0 -> 1.5 s); sweep seed 42 null vs branch: kills / min 27.52 -> 27.65, hits / shots 19.6 -> 21.0 %, 32/32 both (`lead_tmp/sprd/kpm.txt`, taken before MOVR landed).
- For the planner (not carded): FFLN lets one ALREADY-QUEUED salvo leave when a friend walks into the line on that tick (the PS2's fire cycle fires a started salvo whatever the trigger does); seen once in 900 ticks of `a_bot_does_not_fire_through_a_friend`, no hit; the test now skips queued salvos. A fix would cancel the queued salvo when the line is held (ours).
