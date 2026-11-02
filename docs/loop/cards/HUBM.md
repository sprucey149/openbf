---
id: HUBM
title: A hub with no arc for the mover is not a hub: the PS2's locate skips it, the port's does not - TAT1's 525 m dune-sea hub is the hub of two posts and of every unit in the open, so bots hunt 52 % of the time there and 111 are recycled in 8 rounds
milestone: M3
value: 5
phase: build
status: done
deps:
files: engine/crates/core/src/nav.rs,engine/crates/core/src/commander.rs
declares: sim; sweep; arena
needs:
effort:
branch: hubm
resume: landed af03b6c9 2026-10-03
created: 2026-10-02
alpha: yes
---
## Why
On Tatooine Dune Sea (`tat1`, a showcase map) the port's bots are not on the Conquest goal half the time: **52.2 % of
on-foot unit-seconds are `Hunt`** (main 449f006, 8 seeds), a target changes every 30 s (the PS2's bots: one per
224-333 s), and 111 bots are recycled in 8 rounds (48 before card CONT). Audit CONT (2026-10-02) traced it to one
missing test in `NavGraph::locate_on`: the PS2 never puts a unit or a goal point in a hub that has no arc for its
mover. TAT1's hub 104 has a radius of 525 m and no arcs; the port makes it the hub of two of the six posts (cp7, cp2)
and of every unit in the open dune sea, so `commander::plan`'s reach test reads those posts as unreachable from 112 of
115 hubs and everything else as unreachable from hub 104. 15 of the 32 maps have such hubs.

## Evidence (`engine/artifacts/lead_tmp/audit-cont/`; verifier D1-D4 SUPPORTED, `verify/VERDICTS.md`)
- [V] **The PS2's hub mask.** At load each hub's mask is the OR of its arcs' type masks: BF1 `FUN_002663a8`
  (B1:185889-185908, hub+0x14, arc slots at hub+0x18), BF2 `FUN_002eb2b0` (B2:275799-275815, hub+0x10). A hub with no
  arc has mask 0.
- [V] **Locate skips it.** BF1 `FUN_00266678` (B1:185932-185950): the smallest containing hub among those with
  `(hub+0x14 & mask) != 0`; then the arc corridors with `(arc mask & mask) != 0`; its two callers are the path
  request's start and goal (`FUN_00266e50`, B1:186434 / 186437). BF2 `FUN_002eb918` (same test on hub+0x10) and the
  draw's route test `FUN_002ebf70` (B2:276411). BF1's last resort `FUN_00266840` is the nearest ARC whose mask allows
  the mover (read by the auditor, not put to the verifier) - the port's is the nearest hub, any hub.
- [V] **The port has no such test for a soldier**: on 449f006 `locate_on` took any hub with radius > 0 that contains
  the point, and its last step any nearest hub (`core/src/nav.rs`). `analysis/RE_LOG.md` (the planning graph entry)
  records the mask's derivation and "14 hubs with no arcs ... none breaks the loaders", but describes locate without
  the test.
- **WAI2 (a582462, landed while this audit ran) added the filter for VEHICLE types only** (`serves` in `locate_on`:
  `typed = m.ty != SOLDIER && ...`), labelled "OURS, PROVISIONAL ... its locate's type test is not read". It is read
  here: the PS2's test has no soldier exception. What is left is the soldier - and the label.
- **Census** (`reach_err.txt`, the scratch `AUDIT_REACH=1` printout): hubs from which a soldier routes to no post -
  TAT1 0, 1 and 104 (r 525 m, the hub of cp7 and cp2); BF1 END1 9, HOT1 4, NAB1 1, NAB2 1, YAV2 2; BF2 COR1 1,
  DAG1 1, END1 9, GEO1 1, HOT1 8, KAM1 1 (hub 119), MUS1 3, NAB2 1, POL1 4.
- **What it does to the goal** (`tools/port_changes.py` on the observer rows, 8 seeds): TAT1 4,166 in-life target
  changes, 611 of them post -> Hunt for a second -> a post (284 came back to ANOTHER post); BF2 KAM1 56 such blips
  of 254 changes (33 to another post); BF2 END1 90 of 514.
- **Prototype** (`observer_and_hubmask.patch`, `AUDIT_HUBMASK=1`: hubs with no arc for the mover skipped in steps 1
  and 3; switch off = main byte for byte), 8 seeds x 32 maps against main 449f006 (`sw_main`, `sw_hubmask`,
  `tools/rows.py`): 256 / 256 on both; **TAT1 hunting 52.2 -> 1.2 %, recycled 111 -> 26, kills / min 24.2 -> 32.1,
  a target change per 30 -> 87 s** (cp7 -> hub 60, cp2 -> hub 87; 112 of 115 hubs route to all six posts); BF2 KAM1
  blips 56 -> 1, a change per 338 -> 549 s; BF2 HOT1 recycled 24 -> 11, a change per 113 -> 406 s; BF2 END1 152 ->
  404 s; BF1 END1 101 -> 318 s; 17 maps unchanged; total recycled 372 -> 302. Against it: BF2 GEO1 recycled 61 -> 85
  and kills / min 17.3 -> 12.4 (the extra ones at card LLEG's walled cp7 spot, (-84, 264)); BF1 HOT1 recycled 2 -> 13;
  riders killed in destroyed vehicles 771 -> 1018 (BF1 YAV1 33 -> 149).

## Notes
- Filed by audit CONT, 2026-10-02 (`reports/20261002_2300_audit_CONT.md`). No dep on BOTD was set: this is a parity
  defect in where a unit IS on the graph, not a steering rule - but it moves routes on 15 maps, so the planner may
  want it ordered against HUBP / LLEG (LLEG's "TAT1's one-hub dune sea" item is this card's; its last-leg items are
  not).
- The observer (`obj`, `inreg`, `own`, `ai` columns on BOT_SAMPLES rows, an `.own.csv` of post owners) and
  `AUDIT_REACH` are in the same scratch patch; `tools/port_changes.py`, `after_cap.py`, `rows.py`, `causes.py` read
  them. Scratch worktree `openbf-wt/audit-cont-main-scratch` (449f006), target `lead_tmp/audit-cont-main_target`.
- `commander.rs`'s module header still calls `re-conquest-goal.md` "[O] UNAUDITED": it is audited (PARTLY, docs
  e336f2e: the banner and section 10) - correct the header in this build.
- Planner 2026-10-02 23:55: **not held behind BOTD** - the audit left it to the planner. Why: it is the PS2's own rule
  for where a unit IS on the graph (`nav.rs`, which BOTD's sketch lists under "keep, is the PS2's"), with a measured
  prototype, on a showcase map; it is not a steering or brain rule. BOTD's draft (`lead_tmp/botd/bot-design.md`
  section 12) lists it as step 0, "not held". HUBP and LLEG wait for BOTD, so this lands before both. Re-take BOTM's
  TAT1 row after it lands (BOTD's bars were written on the pre-HUBM base). Undo: `loopctl.py set HUBM deps=BOTD`.

## Audit 2026-10-03 (lane 3, `reports/20261003_0250_audit_HUBM.md`): OK -> card LOC3 for what is left
Re-derived before this card's Build section or the report was read. Held: every landed number (clean rebuilds of
13f3da9 and af03b6c print the AIMA and HUBM gate baselines byte for byte at seed 42; finishes 255 -> 256, TAT1
recycled 141 -> 3, kills / min 18.1 -> 32.2, total recycled 347 -> 215, Hunt 42.1 -> 0.6 % with my own 1 Hz reader,
KAM1 blips 79 -> 0, posts on an arcless hub 2 -> 0 of 185), the five tests under seven mutations, and both PS2 rules -
from the instructions of both ELFs and from RAM: every hub's stored mask is the OR of its slot arcs' masks in 346
states, TAT1's hub 104 holds mask 0, and of 1,004 distinct live path objects a transcription reproduces all, without
the hub mask 985 (the 19 are goals inside a hub with no arc for the mover, all stored "nowhere"). The PS2's live
paths to cp2's centre end at hub 87 - the port's new hub for cp2. On main ceb269b the null variant still shows it
(TAT1 recycled 148 -> 4, kills / min 21.8 -> 30.7). Over 32 seeds no map got worse; TAT2 and BF1 BES2 lose their
recycle spots too (unreported), and BF2 HOT1's rise (59 -> 77; the spot at (-264, 0) 25 -> 37) stays inside seed noise
(t +0.6). Not a gain: finishes are 1,023 of 1,024 on both sides over 32 seeds, and the total's fall is TAT1's
(without it 939 -> 903). Unreported: 7 more posts changed hub, all onto an end of the PS2's goal arc (174 -> 183 of
185). Left: the port measures in XZ and takes a fixed nearer end where the PS2 measures in 3-D and takes the end
the route reaches - card LOC3. Verifier 29 / 3 / 0 of 32. Evidence `lead_tmp/audit-hubm/README.md`.
