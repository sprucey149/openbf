---
id: JINK
title: A jump, a roll or a sprint start adds to the unit's aim error, paid from its budget (the PS2's FUN_002ae728: jump +1.0 m, roll and sprint start +2.0, jet +5.0, then the block's decay) - a player or a bot that jinks is harder to hit for 1.3-1.6 s; the port has only the stance factor while the stance lasts (audit of CLSE2, re-soldier-ai 21.2)
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
Card CLSE2 landed "the rest of the PS2's aim error" for BF2's rules. Its audit (2026-10-12, PARTLY) found one more
rule of the same block that neither the RE nor the port has: on the PS2 a unit that JUMPS, ROLLS or starts a
SPRINT gets error added on the spot, and it decays over the next second and a half. It is BF2's dodge: a player
who jumps under fire is missed more, and so is a bot that rolls aside (card REACT's rows). In the port the error
is back at the floor the tick the stance ends.

## Evidence (`analysis/re-soldier-ai.md` 21.2; `engine/artifacts/lead_tmp/audit-clse2/`)
- The rule: `FUN_002ae728(block, kind)` (0x2ae728; the export shows only an unrecovered jump table, the ELF words
  are in 21.2) calls the block's add `FUN_002ae7b8(x, block)` - cut to the budget, taken off the budget, then the
  floor and the 2.0 cap - with x = 2.0 for a roll's entry (`FUN_002471b0`, state 5) and a sprint's entry
  (`FUN_00247440`, state 3, only when the stance really changed), 1.0 for a jump's entry (`FUN_00246ed0`, state 4)
  and 5.0 for the jet (`FUN_002476a0`, states 6 / 7). The block is the unit's own (`*(soldier + 0x324)`), a human's
  and a bot's alike. Two verifiers found it independently from the ELF.
- Live, the human on BF2 Kamino jumping 16 times (`ps2/bf2_kam1_b_human_bumps.txt`): err 0.7789 -> 1.7789 with
  the budget down exactly 1.0000 on 10; the whole budget (0.18-0.33) when it held under 1.0, on 4; +0.9752 to the
  2.0 cap with the budget still down 1.0000, on 1; nothing on an empty budget, 1. His 17 landings: nothing. Bots
  (`ps2/bf2_kam1_a_bumps.txt`): seven jumps and one roll, the same shape.
- The numbers that matter in play (a Normal battle, levels 14 / 11): decay 0.78 / 0.63 a second, floor 0.78 /
  0.97, cap 1.52 / 1.67; the budget refills at half the decay (0.39 / 0.31 a second, at most 5), so one jump
  after 3 quiet seconds takes the whole metre and a second jump at once takes almost nothing.
- The port today: `gunner_foot::stance_factor` x 2 (jump, sprint) and x 3 (roll) inside the cap WHILE the stance
  lasts; `unit::AimBlock` has no event.

## What to do
1. `AimBlock::bump(x, floor)` = the existing `add` made callable, and a call where the sim sees a unit ENTER
   Jump (1.0), Roll (2.0) or Sprint (2.0) under BF2's rules (the tick the stance changes; a human and a bot; no
   jet in the port yet: leave a line in the doc comment). BF1 rounds: nothing (the block does not run there).
2. Extend the audit's replay test into the tree: the human's row pairs of
   `audit-clse2/ps2/bf2_kam1_b_blocks.pkz` as a fixture (err, budget, the velocities, the stance word before and
   after, dt, the skill's rate and floor - `ps2/bump_human.py` prints them) and `AimBlock` reproducing every pair
   within 1e-4, the 16 jumps included.
3. In the same files, the audit's test holes (each a surviving mutation, `audit-clse2/tools/mk_mut.py` G2, G6, N1):
   a BF2 aim test with a MOVING target and a still shooter (the speed term is the target's); the weapon class that
   reaches `seen_aim` at the foot call site and at the seat gunner's (a shotgun's 1.25 and a turret gun's 0.4 seen
   in the offset end to end). And the 80-row RAM test: `clse2/tools/radius_fixture.py` kept only the rows the
   formula already fitted (80 of 112 distinct; 79 distinct tuples) - take moving rows from the audit's captures
   (`ps2/block_replay.py` fits 11,585 of 11,709) and say what share fits.
4. A note for the same session, its own commit: under BF2's rules the port's soldier can lie PRONE (the stance
   cycle stand -> crouch -> prone in `soldier.rs` runs for both games) and its bots do, 0.6-3.5 % of foot time on
   every BF2-rule map; BF2's soldier cannot (its GoProne returns 0, re-stance-roll.md; 0 of 86,652 unit-updates of
   PS2 bots), and a prone target takes the x 2. Gate the cycle by the rules, or file it as its own card if the
   sweep moves much.

## Acceptance
- Tests: a jump adds min(1.0, budget) once at the entry and takes it off the budget; a roll and a sprint start
  min(2.0, budget); the 2.0 cap with the budget still paying the cut amount; nothing at the landing; nothing in a
  BF1 round. Each fails under its own mutation (the bump's size at its point of use, the budget not paid, the
  bump given every tick of the stance).
- The PS2 fixture of step 2: every row pair of the human's capture within 1e-4 (2,591 of 2,591; the think rule
  alone gives 2,571).
- The three holes of step 3 closed: `mk_mut.py` G2, G6 and N1 each fail a test.
- Measured, main against the build, 8 seeds x the 17 BF2-rule maps with the observer (`audit-clse2/tools/sw.sh`,
  `tools/k1.py` + `summ.py`): rounds 136 of 136; kills a minute, contact within 20 m, deaths with an enemy inside
  10 m and damage per aimer-second in the four range bins with their per-seed paired t - reported; a bin that
  moves AWAY from the PS2 past |t| 2.4 is named in the report with its cause. And the maintainer's Kamino scene with a
  jumping human (`clse/tools/scene_seeds.sh`): hits per bolt on him standing against jumping every 3 s, both builds.
