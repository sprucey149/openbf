---
id: LFLK
title: A lost bot flickers on and off the level and paces for 30-90 s (AUDIT RETW): a lost spell ends at the first think on the level and the leg drags the bot back off - BF2 Utapau's pit under hub 60 recycles 0 -> 9 in 88 seeds with RETW's floor walk (today's main 8, RETW switched off 1), BF1 Rhen Var 1's ramp lost time 676 -> 1535 s per 8 seeds; lost episodes over 20 s 56 -> 103 on main; RETW's window rule and level goal have no unit test
status: archived
---

Historical technical record. Recorded state before archival was `todo`.

## Why
AUDIT RETW (2026-10-16, PARTLY; `engine/artifacts/loop/audits.md`): card RETW's floor walk took 'lost' DEATHS 49 -> 3, but the
time bots spend lost did not fall with them, because a lost spell ENDS at the first think on the level (card BNDS: "one think on
ends it") and the bot's leg then drags it back off. Where the level test has an On / Off seam on the bot's way (a pit floor half
On, a ramp behind a barrier), the bot flickers lost / not lost every 1-3 s, each brief return resets the 45 s LOST_RECYCLE, and
it paces in a 4-5 m patch for 30-90 s - invisible to the lost-death bar and, on Rhen Var, to the recycles too. BF2 Utapau is a
showcase map; a player sees a bot run back and forth in the pit under hub 60 for a minute and vanish.

## Evidence (audit dir `engine/artifacts/lead_tmp/audit-retw/`; sweeps one process per map x seed, SWEEP_DEATHS=2)
- Lost time, seeds 1-8 x 32 maps, OBST_CSV 2 Hz rows (`wf/self/epi.py <dir>`; C3 agent's `wf/C3/` agrees to the row):
  confirmed-lost unit-s parent efa2d56 4409.5 -> RETW d5ae05f4 4101.5 (-7 %); today's main 1763d73 with RETW switched off
  (`AUDIT_RETW=3`, switch null-proved: =0 equals main on 128 / 128 rounds) 3739.5 -> main 4153.0 (+11 %). Lost EPISODES per
  life (runs with lost >= 1 merged across gaps <= 10 s) over 20 s: 76 -> 105, and 56 (main, RETW off) -> 103 (main).
- BF1 Rhen Var 1 (bf1_rhn1, x -168..-144, z 24..48 - card LVLB's ramp behind barrier 173): lost s 676.5 -> 1535.0 (main the
  same), episodes over 20 s 7 -> 46, quick re-entries (<= 3 s) 336 -> 905 (agent C3's count, `wf/C3/rhn.py`), 0 deaths and 0 recycles. Trace: land seed 2 unit 31,
  ticks 9120-11460 (`wf/self/rhn_land/bf1_rhn1_s2.err`, TRACE_UNIT=31): leg 99 -> 100 east, Off(0.8-2.5) at x -148..-155,
  the walk back west to x -157.5 (On), the leg east again - 86 level flips in 260 trace rows, 78 s in a 5 m patch.
- BF2 Utapau pit under hub 60 (x -192..-182, z 125..143, floor y 9.4; hub 60's floor is 10.4, 23 m east): recycles in the box
  over seeds 1-88: parent 0 -> RETW 9; main with RETW off 1 -> main 8. Trace on main seed 3 unit 10, ticks 9000-11114
  (`wf/self/uta_main/bf2_uta1_s3.err`, `AUDIT_RETW=16 TRACE_UNIT=10` on the switch build, row = main's): the pit floor reads On
  at z 128-135 and Off(0.99) at z 136-140; legs 68 -> 0 (north) and None -> 60 (east) alternate with lost spells; 22 searches,
  ALL "plan 1" (each a new spell, so RETURN_PLANS never applies), 4.5-8.5 ms each; recycled at (-186.7, 9.4, 136.4).
  Also Utapau cell (-168, 96): lost s 79.5 -> 243 (47 % of the rows standing).
- Tests: of 16 audit mutations of RETW's rules, 12 pass every core test at the landing AND on main
  (`engine/artifacts/lead_tmp/audit-retw/mut/{land,main}_*.log`, `tools/mk_mut.py --list`), among them
  MU22 = the progress window no longer ends the walk (the sweep switch with that rule off: lost deaths 3 -> 47, t +6.8) and
  MU1 = the search's goal without its level test (the doc's named reason, Utapau's pit). One scene test
  (`behave::a_bot_under_a_deck_walks_round_a_corner_to_a_level_hub`) is the only test that notices either RETW part switched off.

## What to do
1. Re-measure on today's main first (`audit-retw/tools/sw1.sh` + `OBST_CSV`, `wf/self/epi.py`).
2. Decide the rule, traced on both spots: e.g. a lost spell that ends on the level keeps its walk to the walk's last point
   (as card RLOOP's pinned walk does) or ends only some metres inside the level, and the leg does not take the bot back into
   cells that read Off within N s; with LVLB's level fix for Rhen Var's ramp (do not double it: LVLB may land first).
3. Tests: the window rule (MU22) and the goal's level test (MU1) each fail a unit test.
4. While there: `Sim::return_path`'s doc says the search goes "never more than BOUND_DROP under the feet"; it does not
   (enforcing it changes 15 of 512 rounds, no lost / recycle change: `sw/swb32` against `sw/land`) - fix the doc or the code.

## Acceptance
- On main, seeds 1-8 x 32 maps with OBST_CSV: lost episodes over 20 s <= 56 (main with RETW off) and confirmed-lost unit-s
  <= 3,740; BF2 Utapau pit-box recycles over seeds 1-88 <= 1; lost deaths over seeds 1-16 <= 3 (RETW's bar kept).
- Mutations MU22 and MU1 (`audit-retw/tools/mk_mut.py`, re-anchored on the branch) each fail at least one test.- 2026-10-16 planner (1016b): ranked against LVLB (both 4, both edit `sim/bounds.rs`, so the picker runs them one after the
  other anyway): **`deps: LVLB`** - LVLB fixes the level test's seam on Rhen Var's ramp (and names a UTA1 ramp), the root of
  part of this flicker; this card then measures what is left on today's main (step 1, `audit-retw/tools/sw1.sh` + OBST_CSV,
  lost TIME and episodes, not deaths) before building. Undo: `set LFLK deps=`.
