---
id: JGATE
title: The jump's gates as on the PS2: a press within 0.3 s of losing the floor still jumps (the port drops it - a moving soldier is off the floor on 1.0-5.7 % of ticks), Cross in a crouch stands up instead of leaping, BF1's stand jump waits 0.5 s in the stance; with the tests JARC's rules still lack (BF2's side factor, the JumpHeight read)
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/soldier.rs
declares: sim; sweep; arena; leak
needs:
effort:
branch: jgate
resume: landed 0cbe7b7d 2026-10-05
created: 2026-10-03
alpha: yes
domain: physics
---
## Why
JARC made the jump's take-off the PS2's; AUDIT JARC (2026-10-03, PARTLY) read the caller JARC left unread and found
three gates where the port still differs, on every map, for the player:
1. **A press off the floor.** BF2 reads the buttons for 0.3 s after the floor bit clears, in the stand and the sprint
   stance; BF1 the same once its stand timer has reached 5 s. The port takes a jump only on a tick with `grounded`
   true, and `jump_edge` is one tick long. A moving soldier in the stand / sprint stance is off the floor on
   1.0-5.7 % of ticks (BF2 Kamino 5.7 %, BF2 Hoth 4.7 %, BF1 Hoth 3.5 %, Naboo Plains 2.8 %, Mos Eisley 1.0 %;
   12 maps x 2 seeds), in runs of 1-5 ticks: a press on one of them does nothing. The JARC report's own "Decisions"
   names the case (a jump on the landing tick of a small hop is dropped).
2. **A crouch.** On the PS2 Cross in a crouch stands the soldier up (both games); the port's crouched soldier leaps at
   full height (6.37 m/s forward with the stick).
3. **BF1's stand jump waits 0.5 s in the stance**; the port's lock after a landing is 0.3 s in both games.
Smaller, same function: the PS2's energy take (`FUN_001e2010`) refuses only a bar whose depleted bit is set and lets
a jump overdraw it; the port refuses whenever energy < cost.

## Evidence (`analysis/re-stance-roll.md` §4.3c; `engine/artifacts/lead_tmp/audit-jarc/README.md`)
- BF2 `FUN_00240bf8`: stand B2:160285-160310 (bit 0x80 + the timer zeroed on the first update off the floor; buttons
  skipped only at timer >= 0.3), sprint B2:160530; crouch B2:160394-160397 (Cross, timer > 0.5 -> slot +0xa0, stand
  up); `FUN_00246ed0` B2:162075-162077 (stance 0 or 3 only). BF1 `FUN_00205cc8` B1:117388-117414, B1:117439.
- Live rows: `lead_tmp/audit-airc/ps2/out/bf2_run_release_2.csv` rows 17-19 and
  `lead_tmp/audit-jarc/ps2/out/bf2_b_sprint_rel.csv` rows 29-30 (a take-off on an update that began off the floor:
  vy 6.9355 = -1.0695 + 8.005, and 7.527 = 0.122 - 0.6 + 8.005); `bf2_crouch_jump.csv` (stance 1 -> 0, vy 0, then a
  jump on the next press); `bf2_crouch_fwd_jump.csv` (Cross 0.2 s into the crouch: nothing); `bf1_crouch_jump.csv`
  (the press at 2.803 s, 0.2 s after a landing: nothing).
- The port: `lead_tmp/audit-jarc/ground/s{1,2}.err` (`tools/ground_tally.patch` on `round_to_outcome`).
- Mutations (`lead_tmp/audit-jarc/mutations.txt`): M8 (JumpHeight not read) and M12 (BF2's side component x the
  forward factor) pass all 36 soldier / step / lost-jump tests.

## Notes
- BF1's roll on Cross with a pure strafe and BF2's AI jump branch stay out (PLAN, locked; JARC's notes).
- A bot presses JUMP only in `brain::lost_jump`, which already asks `grounded`: the window changes the player's jump.

## Planner 2026-10-03 07:15 - value 4 -> 5
- Why: the jump is the maintainer's own ask (AIRC, JARC) and this is the part he can still feel: on BF2 Kamino, the map he
  plays, a moving soldier is off the floor on 5.7 % of ticks, so about 1 jump press in 18 does nothing; the PS2
  forgives 0.3 s. Every other value-5 card is in the held bot chain. Reverse: `loopctl.py set JGATE value=4`.
- From the audit's report (`reports/20261003_0615_audit_JARC.md`): JumpHeight's setter is read now (sqrtf(2 h x 18),
  B2:170972-170976, default 8.0; `analysis/re-stance-roll.md` 4.3c) and the port's conversion is exact for the disc's
  four values, so the PROVISIONAL label on it in `Soldier::from_props` can go in this build, with the test the audit
  says is missing (the JumpHeight read; BF2's side factor).
- BF1BK (value 4) edits the same file (`core/src/soldier.rs`). They are not merged (the batch rule is for value <= 3),
  but the picker will not run them side by side; whoever takes the second rebases on the first.

## Landed 2026-10-05 13:01 - 0cbe7b7 (+ docs 15a828b)
- Report `reports/20261005_1305_card_JGATE.md`. Acceptance run: 5 unit tests, 12 mutations each caught
  (`lead_tmp/jgate/mutations.txt`); ground tally BF2 8 maps 0 dropped of 121,225 in the window, BF1 93 % read and the
  rest the PS2's own 5 s rule (`lead_tmp/jgate/ground`); 8 seeds x 32 maps vs a null 256 -> 256 finished, four BF1
  maps moved (`sw_null`, `sw_jgate`); leak classes down but closed +63, float +5; FEEL_NOTES line written.
- The window is 9 updates (the press on the 9th update that began off the floor), the 10th is refused.
- New hashed field `Soldier::stand_age` (BF1's stand timer); tests now in `core/src/soldier/tests.rs`.
- Left out: BF2 zeroing the stance timer off the floor, the 0.3 s gate on roll / sprint, BF1 prone + jump = get up.
