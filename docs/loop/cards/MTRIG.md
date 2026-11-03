---
id: MTRIG
title: Bots in turrets and tanks hold the trigger on 4.4 % of their seated time where the PS2's are in burst state 3 on 28.4 % (AUDIT BRSTU): since BRSTU the burst is armed 95 % of target time and the gunner's own aim / ready / range gate passes 11 % of it - measure the PS2's real seated trigger and want (FUN_002bb900 clears fire after the machine) and name the gap's cause; it also judges BRSTU's redraw-each-tick deviation
status: archived
---

Historical technical record. Recorded state before archival was `blocked`.

## Why
AUDIT BRSTU (2026-10-21, `engine/artifacts/lead_tmp/audit-brstu/EVIDENCE.md`): since BRSTU a seat's burst is armed (burst running, no pause)
on 94.7 % of a mounted bot's target time, but the bot holds its trigger on only 4.4 % of BF2 mounted samples (5.9 % BF1): the gunner's own
`shot` gate (`ai/gunner.rs` aim: barrel within AIM_TOLERANCE of the aim point, perception's reaction, MaxRange) passes on 11 % of armed
target time. Trigger runs: 1 s+ 28 %, 2.5 s+ 5 %, none over 6 s. The PS2's mounted bots are in burst state 3 on 28.4 % of samples with runs
72 % / 43 % and 91 of 407 over 6 s (`lead_tmp/audit-clse4/ps2/burst_state_mounted.txt`). BRSTU's acceptance compared the PS2's state 3 with
an armed burst, which the port's gunner rarely acts on. Two unknowns decide whether bots in turrets and tanks fire several times less than
the PS2's: (1) PS2 state 3 is not "trigger held" - the want is bit 7 of AI+0xa8 (B2 FUN_002bb1b0 B2:242137-242138, set upstream) and
`FUN_002bb900` clears the fire after the machine; (2) the want's own rule (how often it drops). (2) also judges BRSTU's named deviation
(n redrawn every tick while a burst waits; the PS2 keeps n until the want drops): with the port's `shot` as the want, a knock-in of the PS2's
machine (`AUDIT_BRSTU=4`, `bin/rto_land4b`) matches the landing on BF2 and fires less on BF1 (0.424 against 0.478 shots per target-second;
BF1 Hoth's cannon waits 20 % of its target time, spells to 42 s, against 1 %).
## What to do
1. PS2: read the fire byte AFTER `FUN_002bb900` and the want bit for mounted bots from the captures on disk (`lead_tmp/audit-clse4/tools/ps2_burst_*.py`,
   memory "PS2 burst state in RAM kit"), or a new per-update capture of a BF2 turret / tank gunner: trigger-held share, run lengths, want-held
   share and its drops per second; open the code that sets AI+0xa8 bit 7 for a seated AI.
2. Port: the same table from `sfire` (`lead_tmp/audit-brstu/wf/mtrig/mtrig.py`, `table.py`) and the `shot` gate split by its parts
   (aim off, not ready, out of range).
3. Close with the cause named, or file the build card (the aim gate, the want, or the want reset) with the numbers.
## Acceptance
- The PS2's mounted trigger-held share and run lengths (6 BF2 captures; BF1 if any capture has a mounted bot) beside the port's, and the
  cause of any gap over 2x named with a traced case; the want's drop rate on both sides; a sentence on whether BRSTU's redraw stands in
  for the PS2's kept n.
