---
id: ARVSA
title: The head-to-head's per-team tier is refereed by no test past the spawn skill (AUDIT ARVS): 18 non-equivalent mutations survive every test and the gate - the think's tier, Elite's err cap and the sniper lane each carry tat2's elite>hard verdict alone, the respawn skill moves normal>easy 0.716 -> 0.590, a kill credited to the victim's side REVERSES every pair - and the acceptance's mutation passed by construction
status: archived
issue: 86
---

Historical technical record. Recorded state before archival was `blocked`. Current scope and ownership are on [issue #86](https://github.com/sprucey149/openbf/issues/86).

## Why
AUDIT ARVS (2026-10-20, `engine/artifacts/lead_tmp/audit-arvs/EVIDENCE.md`, `audits.md`): ARVS's arena head-to-head is the referee card TCMP is judged on, and its per-team tier reaches the bots only through readers no test exercises. The only ARVS test checks the accessors and the spawn skill; the gate runs `arena ... all` with the override unset, so it cannot see a reverted reader. Of 33 audit mutations (`lead_tmp/audit-arvs/tools/mk_mut.py --list`), 24 survive every test (core lib 727, the arena's 2):
- verdict words move: R1 (behave.rs:97 think tier from the round), R6 (gunner_foot.rs:466 Elite's err cap), R8 (snipe.rs:301 lane sight) - each alone turns tat2's elite>hard `yes` into a coin flip; R24 (all readers but spawn) gives elite>hard exactly 0.500; A1 (a kill credited to the VICTIM's side) and A2 (orientations swapped) REVERSE every pair; A8 (share denominator +1) reverses flat elite>hard.
- rows move: R3 (style_of), R10 (combat.rs bolt human damage), R13 / R14 (aim blocks by own team / by the round), R16 (spawn.rs:1006 respawn skill: tat2 normal>easy 0.716 -> 0.590), R22 (override never hashed - the test's "an override is hashed" passes because the units' skills differ), A5 (margin sign).
- unreached by the arena (no turrets / vehicles installed, no human in a blast): R5 (gunner.rs:70), R7 (man.rs:151), R9 (blast.rs), A7 (verdict bar).
- equivalent (document, or drop the redundant receiver): R2, R4, R11, R12 (`skill_at` differs only in err_cap), R17, R18 (spec skill overwritten by spawn / respawn).
And the acceptance's mutation as landed ("the higher tier plays the lower: 0.500, t 0") passes by construction (equal orientations make hi_k = lo_k); read literally (R25, the lower tier's skill on both sides) 4 of 6 lines stay `yes`.
## What to build
- One core test per non-equivalent reader under a split tier (round Easy, `team_difficulty` [Some(Easy), Some(Elite)] or the like), with the expected value taken from the other tier's table, not from `difficulty_of`: think tier (view / reaction), style_of's stand draw, seen_aim's err cap (0.5 m, not 1.2), snipe::lane's sight, gunner::aim's reaction, man::hittable's view, bolt and blast human damage, aim blocks decaying by the SHOOTING side's skill, a respawned bot's skill, and the hash moving on an override alone (`relabel = plain.clone(); relabel.team_difficulty = ...; assert_ne!`).
- arena.rs: the beats arithmetic and the verdict as pure functions with a unit test on hand-built rows (orientation, margin sign, denominator, the t bar), and a Fold test feeding one Kill event (side by shooter).
- Optional, the planner's call: the `vs` gate row ARVS proposed (`arena 9000 1 tat2 vs all 8`, ~25 s on a quiet machine, byte-compared) plus `arena 3000 1 flat vs all 24` (~1 s): either would have caught R1, R3, R6, R8, R10, R13, R14, R16, R24, A1, A2, A5, A8.
- Fix the stale docs (unit.rs:253-254 and 762-763, skill.rs base_skill "EVERY bot of every team").
## Acceptance
- Re-run the audit's list (`lead_tmp/audit-arvs/tools/chain_mut.sh` with `mk_mut.py` pointed at the branch): every one of the 18 non-equivalent survivors (R1 R3 R5 R6 R7 R8 R9 R10 R13 R14 R16 R22 R24 A1 A2 A5 A7 A8) fails at least one ci test; R0 still passes; paste the table.
- The null unchanged: `arena 3000 42 flat all` / `arena 9000 42 tat2 all` byte-identical to the gate baseline; `vs all` 24-seed tables byte-identical to `lead_tmp/audit-arvs/runs/t24/land_*.txt` (tests only, no behaviour move).
