---
id: JGAT2
title: The jump's gate counts and the energy bar as the PS2 has them (AUDIT JGATE): BF2's off-floor window is 10 updates (the port's 9 is BF1's), a short bar is OVERDRAWN to minus the dearest cost and un-depletes at EnergyMinSprint with no restore in the air (PS2 -21.3 and 85 updates of no sprint after an overdrawn sprint jump; the port 0.0 and 22), a landing's lock eats presses until L+12 in BF2 / L+20 in BF1 (port L+10 / L+16), and five rule parts still have no test
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT JGATE (2026-10-05, PARTLY) held JGATE's rules and found three numbers in them that are not the PS2's, all in
`core/src/soldier.rs`, all the player's (the PS2's button block runs only for a player's soldier):
1. **BF2's off-floor window is 10 updates, the port takes 9.** JGATE counted "8 x 33.37 ms passes, 9 x does not" -
   33.37 ms is BF1's frame. BF2 steps its timer by 0x3d088888 (one unit under f32 1/30; the EE rounds down), nine
   steps read 0.29999998 and the test is `timer < 0.3f`. BF1's 9 is right. A float timer stepped by 1/30 cannot
   give 10: count updates per game.
2. **The energy bar is overdrawn, not emptied.** The PS2's take clamps at the bar's floor, which the class
   post-load sets to minus the dearest of four costs (-30 on every stock soldier). After a sprint jump on a short
   bar the PS2 soldier cannot sprint for 2.8 s standing (4.1 s walking, derived from the 10 / s rate); the port's
   for 0.7 s / 1.6 s. Same bar,
   same function, also not the PS2's: the bar restores in the air (PS2: not in stances 4 and up), the depleted bit
   clears at a constant 25 (PS2: the class's EnergyMinSprint, 20), idle is "stick at zero" (PS2: horizontal speed
   under 1 m/s), a roll is refused on a short bar (PS2: taken and overdrawn).
3. **After a landing the gate is the lock.** BF2: 0.3 s that masks the jump word on L+1..L+10, and the pad routine
   reads a press on L+11 as "held": the first press that jumps is polled on L+12. BF1: the lock is 10.6 (0.6 s), dead
   on L+1..L+18, first jump on L+20. The port: L+10 and L+16, and JGATE's test asserts BF1 "0.53 s on jumps".

## Evidence (`engine/artifacts/lead_tmp/audit-jgate/README.md`, `verify/VERDICTS.md`; re-stance-roll.md 4.3d)
- Window: `FUN_00240bf8` B2:160119 (timer += dt), B2:160284-160310, the test at 0x24378c-0x2437a8
  (`c.lt.s timer, 0x3e99999a`; bc1fl). Live `ps2/lift_read.py ps2/out bf2_lift`: n = 6..10 jump (15 of 15; n = 10
  vy -5.400 -> +2.005), n = 11 not (0 of 3). BF1 `bf1_old`: 1-9 yes (12 of 12), 10-11 no (0 of 6).
- Bar: `FUN_001e2010` B2:92617-92636, `FUN_001e1e28` B2:92499-92507, `FUN_002517e8` B2:170681-170684 (the four
  costs), `FUN_001e1cb8` (EnergyMinSprint / EnergyOverheat -> cfg[2]), the rate table B2:160209-160225, the roll
  `FUN_002475f0`. RAM: class block [100, -30, 20, 10, 15, 20, 0, 30, 25]. Live `bf2_s_sprint_jump_short_0.csv`:
  bar 10.0 -> -21.333 on the take-off row, held through the 28 jump rows, the bit clears at 20.5, 85 updates after
  the landing row; `bf2_energy_walk_0.csv` rows 140-180 (0.18 m/s against a wall: +0.5 a row).
- Lock: `FUN_00246850` B2:161859-161874, the mask B2:159428-159433, the decrement B2:159773-159776, the pad routine
  `FUN_002b0548` B2:235889-235892; BF1 `FUN_00209018` B1:118505-118517, `FUN_0025e098` B1:179957-179983. Live
  `ps2/land_read.py ps2/out bf2_l` (10 trials) and `bf1_b` (12).
- The port today: `port_tests.log` (scratch tests `tools/audit_tests.rs.txt`); `mutations.txt`.

## Acceptance
Unit tests in `core::soldier` with the PS2's class block (EnergyCostJump 0, JumpSprint 30, Roll 25, MinSprint 20):
1. BF2: a press on the n-th update that began off the floor takes off for n <= 10 and not at 11 (stand and sprint);
   BF1 stays n <= 9. Fails when the two counts are swapped or equal.
2. A sprint jump with the bar at 10.0 two ticks before: the bar reads -21.333 on the take-off tick, depleted; the
   same value on every Jump tick; standing afterwards the bit clears on the first tick at 20 or more, 83-87 ticks
   after the landing tick (PS2 85; the port today 22). EnergyMinSprint is read from the class.
3. The idle rate by horizontal speed under 1 m/s (stick forward against a wall restores at 15 / s); a roll with
   bar 5 and no depleted bit rolls and leaves -20 (code only - label it PROVISIONAL or run the PS2 roll trial
   first: `ps2/gate_probe.py` has the pieces).
4. The first tick a jump press is taken after the landing tick L: BF2 L+12, BF1 L+20 (the port today L+10 / L+16);
   the "0.53 s" assertion goes.
5. Each of the five untested parts fails a test (`tools/mutate.py`: FREE, PLANAR, AGE3, FALL-OLD, LOCK).
Then 8 seeds against a null beside the change (JGATE moved no BF2 map; say which maps the bar moves - bots
sprint), the leak census per class, one FEEL_NOTES line. Declares sim; sweep; arena; leak.

## Notes
- Unowned leftovers 15a828b lists as "not ported"; take the cheap ones here, name the rest in the report:
  (a) BF2 zeroes the stance timer on the first update off the floor - live, ONE update off a floor seam restarts
  the sprint's speed ramp (10.50 -> 7.67 m/s; the jump 5 updates later leaves at 11.27, not 13.65 m/s:
  `bf2_s_sprint_jump_short_0.csv` rows 26-34); (b) roll / crouch / sprint presses are skipped with the jump once
  0.3 s off the floor (the port rolls in mid-air 0.5 s into a fall); (c) BF1's prone rises on Cross 0.5 s in
  (`FUN_00208f60`; no PS2 row - no prone key was found on the pad scan).
- The AI's jump (`FUN_00249f80`) calls the jump slot itself and pays no energy; the port's bots press JUMP like a
  player (JARC's note), so the bar's floor reaches a bot that sprint-jumps.
- The verifier's list of other takers of the bar (`FUN_002476a0`: class+0x88c in the air - the jet jump, card
  JETP; weapons through slot +0x4c) is in `verify/VERDICTS.md`.
- BF1BK shares `soldier.rs`: one after the other.
- 2026-10-06 planner: `alpha: yes` - JGATE was an alpha card (the maintainer's dropped jumps) and these are the numbers of the same rules; it now goes before the untagged fours. The one a player can feel is item 2 (a sprint jump on a short bar: the PS2 leaves you unable to sprint for 2.8 s, the port for 0.7 s). Undo: `loopctl.py set JGAT2 alpha=`.
